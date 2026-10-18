//! Last room and "Iniciar com o sistema e voltar a esta sala", as the desktop web client:
//! the name and room come back on the join form, and with autostart on the app rejoins by
//! itself (hidden in the tray when Windows started it).
use std::path::PathBuf;

use windows::{
    Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW},
    core::{PCWSTR, w},
};

/// Passed by the Run entry, like the Tauri login item, so a login launch stays in the tray.
pub const AUTOSTART_ARG: &str = "--autostart";
const RUN_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
// Not the Tauri app's name, so both can be installed and start on their own.
const RUN_VALUE: PCWSTR = w!("Screenshare-native");

/// The first instance's "show yourself" event, polled by the UI (a raw handle, never closed).
static SHOW_EVENT: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

/// One app at a time, like the Tauri single-instance plugin: false means another instance was
/// already running, has been asked to show its window, and this one should quit.
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
}

fn path(file: &str) -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("Screenshare-native").join(file))
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

fn run_key(access: windows::Win32::System::Registry::REG_SAM_FLAGS) -> Option<HKEY> {
    let mut key = HKEY::default();
    // SAFETY: plain out-parameter; the key is closed by the caller.
    unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, None, access, &mut key) }.is_ok().then_some(key)
}

pub fn autostart_enabled() -> bool {
    let Some(key) = run_key(KEY_READ) else { return false };
    let found = unsafe { RegQueryValueExW(key, RUN_VALUE, None, None, None, None) }.is_ok();
    unsafe {
        let _ = RegCloseKey(key);
    }
    found
}

/// The Run entry points at this exe with `--autostart`; off removes it.
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
