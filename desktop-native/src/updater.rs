//! Updates (docs/native-desktop-plan.md, next steps): the newest native-v* release on GitHub. The installed
//! Windows app downloads its setup, checks SHA256SUMS and runs it silently; the portable exe and macOS
//! open the release page. Native releases are never "latest" (the Tauri updater reads that), so this
//! lists them and picks the highest tag.
use serde::Deserialize;

const RELEASES: &str = "https://api.github.com/repos/RafaelrainBR/screenshare-kotlin-js/releases?per_page=50";

pub type Version = (u64, u64, u64);

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: Version,
    pub page: String,
    /// The setup and SHA256SUMS download links, when the release has both.
    setup: Option<(String, String, String)>,
}

impl Release {
    pub fn label(&self) -> String {
        let (major, minor, patch) = self.version;
        format!("{major}.{minor}.{patch}")
    }
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    draft: bool,
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

/// native-vX.Y.Z; tags with a suffix (-rc.1) are not offered.
fn parse(tag: &str) -> Option<Version> {
    let mut parts = tag.strip_prefix("native-v")?.split('.').map(|part| part.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

/// The tag this build was released as (release-native.yml passes it as SENTRY_RELEASE); local builds have none and never update.
pub fn current() -> Option<Version> {
    parse(option_env!("SENTRY_RELEASE")?)
}

fn newest(releases: Vec<GhRelease>, current: Version) -> Option<Release> {
    let (version, release) = releases.into_iter().filter(|release| !release.draft).filter_map(|release| Some((parse(&release.tag_name)?, release))).max_by_key(|(version, _)| *version)?;
    if version <= current {
        return None;
    }
    let (major, minor, patch) = version;
    let file = format!("Screenshare-native_{major}.{minor}.{patch}_x64-setup.exe");
    let url = |name: &str| release.assets.iter().find(|asset| asset.name == name).map(|asset| asset.browser_download_url.clone());
    let setup = url(&file).zip(url("SHA256SUMS")).map(|(setup, sums)| (file, setup, sums));
    Some(Release { version, page: release.html_url, setup })
}

fn client() -> Result<reqwest::Client, String> {
    // GitHub's API refuses requests without a User-Agent.
    reqwest::Client::builder().user_agent("screenshare-native").build().map_err(|error| error.to_string())
}

/// A release newer than this build, if any. Errors (offline, rate limit) just mean "try later".
pub async fn check() -> Option<Release> {
    let current = current()?;
    let fetch = async {
        let response = client()?.get(RELEASES).header("Accept", "application/vnd.github+json").send().await.map_err(|error| error.to_string())?;
        response.error_for_status().map_err(|error| error.to_string())?.json::<Vec<GhRelease>>().await.map_err(|error| error.to_string())
    };
    match fetch.await {
        Ok(releases) => newest(releases, current),
        Err(error) => {
            sentry::logger_warn!("update check failed: {}", error.as_str());
            None
        }
    }
}

/// Installed by the setup (its uninstaller sits next to the exe), so the setup can replace it.
pub fn installed() -> bool {
    cfg!(windows) && std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join("unins000.exe").exists())).unwrap_or(false)
}

pub fn can_install(release: &Release) -> bool {
    release.setup.is_some() && installed()
}

fn sha256_hex(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes).as_ref().iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The hash `sha256sum` wrote for `file` (lines of "<hex>  <name>", binary mode adds a '*').
fn expected_hash<'a>(sums: &'a str, file: &str) -> Option<&'a str> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.split_once(char::is_whitespace)?;
        (name.trim_start().trim_start_matches('*') == file).then_some(hash)
    })
}

/// Downloads and checks the setup, then starts it silently; the caller quits so the exe can be replaced.
/// The setup opens the app again when it finishes (installer.iss).
pub async fn install(release: Release) -> Result<(), String> {
    let (file, setup, sums) = release.setup.ok_or("Esta versão não tem instalador.")?;
    let client = client()?;
    let get = |url: String| {
        let client = client.clone();
        async move { client.get(url).send().await.and_then(|response| response.error_for_status()).map_err(|error| error.to_string())?.bytes().await.map_err(|error| error.to_string()) }
    };
    let sums = String::from_utf8_lossy(&get(sums).await?).into_owned();
    let expected = expected_hash(&sums, &file).ok_or("SHA256SUMS sem o instalador.")?.to_ascii_lowercase();
    let bytes = get(setup).await?;
    if sha256_hex(&bytes) != expected {
        return Err("O instalador baixado não confere com o SHA256SUMS.".into());
    }
    let path = std::env::temp_dir().join(&file);
    std::fs::write(&path, &bytes).map_err(|error| error.to_string())?;
    std::process::Command::new(&path).args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"]).spawn().map_err(|error| error.to_string())?;
    Ok(())
}

/// The portable exe and macOS: the release page, where the user downloads it.
pub fn open_page(release: &Release) {
    #[cfg(windows)]
    let opener = "explorer";
    #[cfg(not(windows))]
    let opener = "open";
    let _ = std::process::Command::new(opener).arg(&release.page).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, assets: &[&str]) -> GhRelease {
        GhRelease {
            tag_name: tag.into(),
            html_url: format!("https://example/{tag}"),
            draft: false,
            assets: assets.iter().map(|name| GhAsset { name: (*name).into(), browser_download_url: format!("https://example/{tag}/{name}") }).collect(),
        }
    }

    #[test]
    fn picks_the_highest_native_tag_above_the_current_one() {
        assert_eq!(parse("native-v0.1.18"), Some((0, 1, 18)));
        assert_eq!(parse("native-v0.1.18-rc.1"), None);
        assert_eq!(parse("v1.4.0"), None);
        assert_eq!(parse("native-v0.1"), None);
        let list = || vec![release("v9.0.0", &[]), release("native-v0.1.9", &[]), release("native-v0.1.20", &["Screenshare-native_0.1.20_x64-setup.exe", "SHA256SUMS"]), release("native-v0.1.18", &[])];
        let newest = newest(list(), (0, 1, 17)).unwrap();
        assert_eq!((newest.label(), newest.page.as_str()), ("0.1.20".into(), "https://example/native-v0.1.20"));
        assert_eq!(newest.setup.unwrap().1, "https://example/native-v0.1.20/Screenshare-native_0.1.20_x64-setup.exe");
        assert_eq!(super::newest(list(), (0, 1, 20)), None);
        let mut draft = release("native-v0.2.0", &[]);
        draft.draft = true;
        assert_eq!(super::newest(vec![draft, release("native-v0.1.19", &[])], (0, 1, 17)).map(|r| (r.label(), r.setup)), Some(("0.1.19".into(), None)));
    }

    #[test]
    fn finds_the_setup_hash_in_sha256sums() {
        let sums = "aa11  Screenshare-native_0.1.20_x64-portable.exe\nBB22 *Screenshare-native_0.1.20_x64-setup.exe\n";
        assert_eq!(expected_hash(sums, "Screenshare-native_0.1.20_x64-setup.exe"), Some("BB22"));
        assert_eq!(expected_hash(sums, "SHA256SUMS"), None);
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
