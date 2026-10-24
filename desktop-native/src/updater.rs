//! Updates (docs/native-desktop-plan.md, next steps): the newest native-v* release on GitHub. The app
//! downloads its own package, checks it against SHA256SUMS and replaces itself:
//! - installed on Windows: runs the setup silently, which opens the app again (installer.iss);
//! - portable exe: renames itself to .old (Windows allows that while running) and writes the new exe;
//! - macOS: unpacks the tar.gz next to the .app and swaps the bundles.
//! Native releases are never "latest" (the Tauri updater reads that), so this lists them and picks the highest tag.
use std::path::{Path, PathBuf};

use serde::Deserialize;

const RELEASES: &str = "https://api.github.com/repos/RafaelrainBR/screenshare-kotlin-js/releases?per_page=50";
/// Given to the new exe so it waits for this process to end before the single-instance check.
const REPLACES_ARG: &str = "--replaces";
/// Next to the .app on macOS: the unpacked bundle, then the old one, removed at the next launch.
#[cfg(target_os = "macos")]
const STAGING: &str = ".screenshare-native-update";

pub type Version = (u64, u64, u64);

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: Version,
    pub page: String,
    /// This system's package: file name, its link and the SHA256SUMS link, when the release has both.
    package: Option<(String, String, String)>,
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

/// The release asset this copy of the app updates from (release-native.yml names them).
fn package(version: &str) -> String {
    if cfg!(target_os = "macos") {
        "Screenshare-native-macos-arm64.tar.gz".into()
    } else if installed() {
        format!("Screenshare-native_{version}_x64-setup.exe")
    } else {
        format!("Screenshare-native_{version}_x64-portable.exe")
    }
}

fn newest(releases: Vec<GhRelease>, current: Version, package: impl Fn(&str) -> String) -> Option<Release> {
    let (version, release) = releases.into_iter().filter(|release| !release.draft).filter_map(|release| Some((parse(&release.tag_name)?, release))).max_by_key(|(version, _)| *version)?;
    if version <= current {
        return None;
    }
    let (major, minor, patch) = version;
    let file = package(&format!("{major}.{minor}.{patch}"));
    let url = |name: &str| release.assets.iter().find(|asset| asset.name == name).map(|asset| asset.browser_download_url.clone());
    let package = url(&file).zip(url("SHA256SUMS")).map(|(link, sums)| (file, link, sums));
    Some(Release { version, page: release.html_url, package })
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
        Ok(releases) => newest(releases, current, package),
        Err(error) => {
            crate::telemetry::warn(&format!("update check failed: {error}"));
            None
        }
    }
}

/// Installed by the setup (its uninstaller sits next to the exe), so the setup can replace it.
fn installed() -> bool {
    cfg!(windows) && std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join("unins000.exe").exists())).unwrap_or(false)
}

/// The release has this system's package (otherwise the update opens its page).
pub fn can_install(release: &Release) -> bool {
    (cfg!(windows) || cfg!(target_os = "macos")) && release.package.is_some()
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

fn io(error: std::io::Error) -> String {
    error.to_string()
}

/// Downloads and checks this system's package, puts the new version in place and starts it; the caller
/// quits right after, and the new process waits for that (`wait_for_replaced`).
pub async fn install(release: Release) -> Result<(), String> {
    let (file, link, sums) = release.package.ok_or("Esta versão não tem o pacote deste sistema.")?;
    let client = client()?;
    let get = |url: String| {
        let client = client.clone();
        async move { client.get(url).send().await.and_then(|response| response.error_for_status()).map_err(|error| error.to_string())?.bytes().await.map_err(|error| error.to_string()) }
    };
    let sums = String::from_utf8_lossy(&get(sums).await?).into_owned();
    let expected = expected_hash(&sums, &file).ok_or("SHA256SUMS sem o pacote.")?.to_ascii_lowercase();
    let bytes = get(link).await?;
    if sha256_hex(&bytes) != expected {
        return Err("O pacote baixado não confere com o SHA256SUMS.".into());
    }
    replace(&file, &bytes)
}

#[cfg(windows)]
fn replace(file: &str, bytes: &[u8]) -> Result<(), String> {
    if installed() {
        let path = std::env::temp_dir().join(file);
        std::fs::write(&path, bytes).map_err(io)?;
        std::process::Command::new(&path).args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"]).spawn().map_err(io)?;
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(io)?;
    let old = old_exe(&exe);
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&exe, &old).map_err(io)?;
    if let Err(error) = std::fs::write(&exe, bytes) {
        let _ = std::fs::rename(&old, &exe);
        return Err(io(error));
    }
    std::process::Command::new(&exe).args([REPLACES_ARG, &std::process::id().to_string()]).spawn().map_err(io)?;
    Ok(())
}

#[cfg(windows)]
fn old_exe(exe: &Path) -> PathBuf {
    exe.with_extension("exe.old")
}

#[cfg(target_os = "macos")]
fn replace(file: &str, bytes: &[u8]) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(io)?;
    // …/Screenshare nativo.app/Contents/MacOS/Screenshare-native
    let app = exe.ancestors().nth(3).filter(|app| app.extension().is_some_and(|ext| ext == "app")).ok_or("O app não está num pacote .app.")?;
    let staging = app.parent().ok_or("pasta do app")?.join(STAGING);
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(io)?;
    let archive = staging.join(file);
    std::fs::write(&archive, bytes).map_err(io)?;
    // Downloaded by the app, not a browser: no quarantine attribute, so Gatekeeper lets the ad hoc bundle run.
    let unpacked = std::process::Command::new("tar").arg("-xzf").arg(&archive).arg("-C").arg(&staging).status().map_err(io)?;
    let new = staging.join("Screenshare nativo.app");
    if !unpacked.success() || !new.is_dir() {
        return Err("Pacote do macOS inválido.".into());
    }
    let old = staging.join("old.app");
    std::fs::rename(app, &old).map_err(io)?;
    if let Err(error) = std::fs::rename(&new, app) {
        let _ = std::fs::rename(&old, app);
        return Err(io(error));
    }
    // -n: a new instance, even though this one is still quitting.
    std::process::Command::new("open").arg("-n").arg(app).args(["--args", REPLACES_ARG, &std::process::id().to_string()]).spawn().map_err(io)?;
    Ok(())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn replace(_: &str, _: &[u8]) -> Result<(), String> {
    Err("Atualização não suportada neste sistema.".into())
}

/// Leftovers of the last update: the portable's .old exe, macOS's staging folder.
pub fn clean_up() {
    let Ok(exe) = std::env::current_exe() else { return };
    #[cfg(windows)]
    let _ = std::fs::remove_file(old_exe(&exe));
    #[cfg(target_os = "macos")]
    if let Some(parent) = exe.ancestors().nth(4) {
        let _ = std::fs::remove_dir_all(parent.join(STAGING));
    }
    let _ = exe;
}

/// Started by an update: waits (up to 10 s) for the old process, so the single-instance check does not
/// send this launch to the app that is quitting.
pub fn wait_for_replaced() {
    let mut args = std::env::args();
    let Some(pid) = args.position(|arg| arg == REPLACES_ARG).and_then(|_| args.next()).and_then(|pid| pid.parse::<u32>().ok()) else { return };
    #[cfg(windows)]
    {
        use windows::Win32::{
            Foundation::CloseHandle,
            System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
        };
        // SAFETY: a handle opened here, waited on and closed.
        if let Ok(process) = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) } {
            unsafe {
                WaitForSingleObject(process, 10_000);
                let _ = CloseHandle(process);
            }
        }
    }
    #[cfg(target_os = "macos")]
    for _ in 0..100 {
        // SAFETY: signal 0 only checks that the process exists.
        if unsafe { libc::kill(pid as i32, 0) } != 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// When the release has no package for this system, or installing failed: its page.
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
        let setup = |version: &str| format!("Screenshare-native_{version}_x64-setup.exe");
        let list = || vec![release("v9.0.0", &[]), release("native-v0.1.9", &[]), release("native-v0.1.20", &["Screenshare-native_0.1.20_x64-setup.exe", "SHA256SUMS"]), release("native-v0.1.18", &[])];
        let newest = newest(list(), (0, 1, 17), setup).unwrap();
        assert_eq!((newest.label(), newest.page.as_str()), ("0.1.20".into(), "https://example/native-v0.1.20"));
        assert_eq!(newest.package.unwrap().1, "https://example/native-v0.1.20/Screenshare-native_0.1.20_x64-setup.exe");
        assert_eq!(super::newest(list(), (0, 1, 20), setup), None);
        // A package this release lacks: only its page.
        assert_eq!(super::newest(list(), (0, 1, 17), |_| "Screenshare-native-macos-arm64.tar.gz".into()).unwrap().package, None);
        let mut draft = release("native-v0.2.0", &[]);
        draft.draft = true;
        assert_eq!(super::newest(vec![draft, release("native-v0.1.19", &[])], (0, 1, 17), setup).map(|r| (r.label(), r.package)), Some(("0.1.19".into(), None)));
    }

    #[test]
    fn finds_the_package_hash_in_sha256sums() {
        let sums = "aa11  Screenshare-native_0.1.20_x64-portable.exe\nBB22 *Screenshare-native_0.1.20_x64-setup.exe\n";
        assert_eq!(expected_hash(sums, "Screenshare-native_0.1.20_x64-setup.exe"), Some("BB22"));
        assert_eq!(expected_hash(sums, "SHA256SUMS"), None);
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn this_copy_updates_from_its_own_package() {
        // The test exe has no uninstaller next to it: portable on Windows.
        let expected = if cfg!(target_os = "macos") { "Screenshare-native-macos-arm64.tar.gz" } else { "Screenshare-native_0.1.20_x64-portable.exe" };
        assert_eq!(package("0.1.20"), expected);
    }
}
