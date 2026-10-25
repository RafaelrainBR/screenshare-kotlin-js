//! Last room and "Iniciar com o sistema e voltar a esta sala", as the desktop web client:
//! the name and room come back on the join form, and with autostart on the app rejoins by
//! itself (hidden in the tray when Windows started it).
use std::path::PathBuf;

#[cfg(windows)]
use windows::{
    Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW},
    core::{PCWSTR, w},
};

/// Passed by the Run entry, like the Tauri login item, so a login launch stays in the tray.
pub const AUTOSTART_ARG: &str = "--autostart";
#[cfg(windows)]
const RUN_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
// Not the Tauri app's name, so both can be installed and start on their own.
#[cfg(windows)]
const RUN_VALUE: PCWSTR = w!("Screenshare-native");

/// The first instance's "show yourself" event, polled by the UI (a raw handle, never closed).
#[cfg(windows)]
static SHOW_EVENT: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

/// One app at a time, like the Tauri single-instance plugin: false means another instance was
/// already running, has been asked to show its window, and this one should quit.
#[cfg(windows)]
pub fn first_instance() -> bool {
    use windows::Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
        System::Threading::{CreateEventW, SetEvent},
    };
    // SAFETY: a named auto-reset event; the handle lives as long as the process.
    let Ok(event) = (unsafe { CreateEventW(None, false, false, w!(r"Local\Screenshare-native-show")) }) else {
        return true;
    };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        let _ = unsafe { SetEvent(event) };
        return false;
    }
    let _ = SHOW_EVENT.set(event.0 as usize);
    true
}

/// A second launch asked this instance to show its window.
#[cfg(windows)]
pub fn show_requested() -> bool {
    use windows::Win32::{Foundation::{HANDLE, WAIT_OBJECT_0}, System::Threading::WaitForSingleObject};
    SHOW_EVENT.get().is_some_and(|event| unsafe { WaitForSingleObject(HANDLE(*event as *mut _), 0) } == WAIT_OBJECT_0)
}

pub fn launched_at_login() -> bool {
    std::env::args().any(|arg| arg == AUTOSTART_ARG)
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct LastRoom {
    pub name: String,
    pub room: String,
}

/// screenshare.outputMuted and screenshare.outputDeviceId in the web's localStorage: the room's
/// sound plays unless it was muted last time, on the output picked last time.
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct AudioPrefs {
    pub output_muted: bool,
    pub output_device: Option<String>,
    /// Off rather than on, so a missing field or file keeps the room tones on.
    #[serde(default)]
    pub room_sounds_off: bool,
}

/// %APPDATA%\Screenshare-native on Windows, ~/Library/Application Support/Screenshare-native on macOS.
pub fn data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = PathBuf::from(std::env::var_os("APPDATA")?);
    #[cfg(target_os = "macos")]
    let base = PathBuf::from(std::env::var_os("HOME")?).join("Library").join("Application Support");
    Some(base.join("Screenshare-native"))
}

fn path(file: &str) -> Option<PathBuf> {
    Some(data_dir()?.join(file))
}

fn read<T: serde::de::DeserializeOwned + Default>(file: &str) -> T {
    path(file).and_then(|path| std::fs::read_to_string(path).ok()).and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default()
}

fn write(file: &str, value: &impl serde::Serialize) {
    if let (Some(path), Ok(json)) = (path(file), serde_json::to_string(value)) {
        let _ = path.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(path, json);
    }
}

pub fn last_room() -> LastRoom {
    read("last-room.json")
}

pub fn save_last_room(name: &str, room: &str) {
    write("last-room.json", &LastRoom { name: name.into(), room: room.into() });
}

/// screenshare.volume.user.<name>: each person's voice volume, 0..=100, by name.
pub fn voice_volumes() -> std::collections::HashMap<String, u8> {
    read("volumes.json")
}

pub fn save_voice_volumes(volumes: &std::collections::HashMap<String, u8>) {
    write("volumes.json", volumes);
}

pub fn audio_prefs() -> AudioPrefs {
    read("audio.json")
}

pub fn save_audio_prefs(prefs: &AudioPrefs) {
    write("audio.json", prefs);
}

#[cfg(windows)]
fn run_key(access: windows::Win32::System::Registry::REG_SAM_FLAGS) -> Option<HKEY> {
    let mut key = HKEY::default();
    // SAFETY: plain out-parameter; the key is closed by the caller.
    unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, None, access, &mut key) }.is_ok().then_some(key)
}

#[cfg(windows)]
pub fn autostart_enabled() -> bool {
    let Some(key) = run_key(KEY_READ) else { return false };
    let found = unsafe { RegQueryValueExW(key, RUN_VALUE, None, None, None, None) }.is_ok();
    unsafe {
        let _ = RegCloseKey(key);
    }
    found
}

/// The Run entry points at this exe with `--autostart`; off removes it.
#[cfg(windows)]
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    let key = run_key(KEY_SET_VALUE).ok_or("Não foi possível abrir o registro do Windows.")?;
    let result = if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let command = format!("\"{}\" {AUTOSTART_ARG}", exe.display());
        let wide: Vec<u16> = command.encode_utf16().chain([0]).collect();
        let bytes = unsafe { std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * 2) };
        unsafe { RegSetValueExW(key, RUN_VALUE, None, REG_SZ, Some(bytes)) }.ok()
    } else {
        // Already absent is fine.
        let result = unsafe { RegDeleteValueW(key, RUN_VALUE) };
        if result.0 == 2 { Ok(()) } else { result.ok() }
    };
    unsafe {
        let _ = RegCloseKey(key);
    }
    result.map_err(|error| format!("Não foi possível alterar o início automático do sistema ({error})."))
}

// macOS: LaunchServices already keeps one copy of an .app running and brings it forward when it is
// opened again, so there is no second instance to signal.
#[cfg(target_os = "macos")]
pub fn first_instance() -> bool {
    true
}

#[cfg(target_os = "macos")]
pub fn show_requested() -> bool {
    false
}

/// The login item, as a LaunchAgent: this executable with `--autostart` at login.
#[cfg(target_os = "macos")]
fn launch_agent() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("HOME")?).join("Library/LaunchAgents/com.screenshare.native.plist"))
}

#[cfg(target_os = "macos")]
pub fn autostart_enabled() -> bool {
    launch_agent().is_some_and(|path| path.exists())
}

#[cfg(target_os = "macos")]
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    let path = launch_agent().ok_or("Não foi possível encontrar a pasta do usuário.")?;
    if !enabled {
        // Already absent is fine.
        return match std::fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(format!("Não foi possível alterar o início automático do sistema ({error}).")),
            _ => Ok(()),
        };
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let escape = |text: &str| text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let plist = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n\t<key>Label</key>\n\t<string>com.screenshare.native</string>\n\t<key>ProgramArguments</key>\n\t<array>\n\t\t<string>{}</string>\n\t\t<string>{AUTOSTART_ARG}</string>\n\t</array>\n\t<key>RunAtLoad</key>\n\t<true/>\n</dict>\n</plist>\n",
        escape(&exe.display().to_string())
    );
    let _ = path.parent().map(std::fs::create_dir_all);
    std::fs::write(&path, plist).map_err(|error| format!("Não foi possível alterar o início automático do sistema ({error})."))
}

/// The join screen's recent and favorite rooms (the web's screenshare.recentRooms).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct RecentRoom {
    pub id: String,
    #[serde(default)]
    pub favorite: bool,
}

const MAX_RECENT: usize = 5;
const MAX_ROOM_ID: usize = 64;

pub fn recent_rooms() -> Vec<RecentRoom> {
    read("rooms.json")
}

pub fn save_recent_rooms(rooms: &[RecentRoom]) {
    write("rooms.json", &rooms);
}

/// `id` goes to the front of the recent ones; favorites keep their place and never age out.
pub fn remember_room(rooms: &[RecentRoom], id: &str) -> Vec<RecentRoom> {
    if id.trim().is_empty() || id.len() > MAX_ROOM_ID || rooms.iter().any(|room| room.id == id && room.favorite) {
        return rooms.to_vec();
    }
    let others = rooms.iter().filter(|room| room.id != id);
    let favorites = others.clone().filter(|room| room.favorite).cloned();
    let recent = std::iter::once(RecentRoom { id: id.into(), favorite: false }).chain(others.filter(|room| !room.favorite).cloned()).take(MAX_RECENT);
    favorites.chain(recent).collect()
}

/// Unstarring keeps the room as the newest recent one.
pub fn toggle_favorite(rooms: &[RecentRoom], id: &str) -> Vec<RecentRoom> {
    let Some(room) = rooms.iter().find(|room| room.id == id) else { return rooms.to_vec() };
    let others: Vec<_> = rooms.iter().filter(|room| room.id != id).cloned().collect();
    if room.favorite {
        remember_room(&others, id)
    } else {
        let starred = RecentRoom { id: id.into(), favorite: true };
        others.iter().filter(|room| room.favorite).cloned().chain([starred]).chain(others.iter().filter(|room| !room.favorite).cloned()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(rooms: &[RecentRoom]) -> Vec<String> {
        rooms.iter().map(|room| if room.favorite { format!("*{}", room.id) } else { room.id.clone() }).collect()
    }

    fn remember_all(rooms: Vec<RecentRoom>, ids: &[&str]) -> Vec<RecentRoom> {
        ids.iter().fold(rooms, |rooms, id| remember_room(&rooms, id))
    }

    // The same cases as the web's RecentRoomsTest.
    #[test]
    fn the_newest_room_leads_the_recent_ones_and_only_five_stay() {
        let rooms = remember_all(vec![], &["a", "b", "c", "d", "e", "f"]);
        assert_eq!(ids(&rooms), ["f", "e", "d", "c", "b"]);
        assert_eq!(ids(&remember_room(&rooms, "c")), ["c", "f", "e", "d", "b"]);
        assert_eq!(ids(&remember_room(&rooms, " ")), ids(&rooms));
        assert_eq!(ids(&remember_room(&rooms, &"x".repeat(65))), ids(&rooms));
    }

    #[test]
    fn favorites_stay_at_the_front_and_never_age_out() {
        let rooms = toggle_favorite(&remember_all(vec![], &["a", "b", "c"]), "a");
        assert_eq!(ids(&rooms), ["*a", "c", "b"]);
        let rooms = remember_all(rooms, &["d", "e", "f", "g", "h", "a"]);
        assert_eq!(ids(&rooms), ["*a", "h", "g", "f", "e", "d"]);
        assert_eq!(ids(&toggle_favorite(&rooms, "a")), ["a", "h", "g", "f", "e"]);
    }
}
