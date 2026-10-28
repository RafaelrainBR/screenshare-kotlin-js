//! Sentry (docs/native-desktop-plan.md, F7): panics, errors and logs, only from builds given a DSN.
//! Events carry no name, room or machine: the room and the username stay out of every message.
use std::sync::Arc;

/// The release workflow builds with SENTRY_DSN_NATIVE and SENTRY_RELEASE (the tag); local builds
/// and forks have no DSN and send nothing.
pub fn init() -> Option<sentry::ClientInitGuard> {
    let dsn = option_env!("SENTRY_DSN_NATIVE").filter(|dsn| !dsn.is_empty())?;
    let mut options = sentry::ClientOptions::default();
    options.release = Some(option_env!("SENTRY_RELEASE").unwrap_or(concat!("screenshare-native@", env!("CARGO_PKG_VERSION"))).into());
    options.environment = Some(if cfg!(debug_assertions) { "development" } else { "production" }.into());
    // Crash-free sessions per release: one per launch.
    options.auto_session_tracking = true;
    options.session_mode = sentry::SessionMode::Application;
    // A crash in C++ (libwebrtc) kills the process before Rust can report it: a second copy of this
    // exe, started inside sentry::init and never returning from it, writes the minidump and sends it.
    options.integrations.push(Arc::new(sentry::integrations::minidump::MinidumpIntegration::new()));
    options.before_send = Some(Arc::new(|mut event| {
        // The PC's name and anything user-like never leave the machine.
        event.server_name = None;
        event.user = None;
        Some(event)
    }));
    let guard = sentry::init((dsn, options));
    guard.is_enabled().then_some(guard)
}

/// Local log for when the network or Sentry is down: the release exe has no console, so stderr
/// (every eprintln!, panic message and engine line) goes to logs/screenshare.log in the data dir.
/// Debug builds keep their console.
pub fn redirect_stderr() {
    if cfg!(debug_assertions) {
        return;
    }
    let Some(dir) = logs_dir() else { return };
    let file = dir.join("screenshare.log");
    // shortcut: rotates only at launch (1 MB, one old file); rotate while running if a long call fills it.
    if std::fs::metadata(&file).is_ok_and(|meta| meta.len() > 1 << 20) {
        let _ = std::fs::rename(&file, dir.join("screenshare.old.log"));
    }
    let Ok(file) = std::fs::OpenOptions::new().create(true).append(true).open(file) else { return };
    #[cfg(windows)]
    {
        use std::os::windows::io::IntoRawHandle;
        use windows::Win32::{Foundation::HANDLE, System::Console::{STD_ERROR_HANDLE, SetStdHandle}};
        // SAFETY: the handle is leaked on purpose and stays valid for the life of the process.
        let _ = unsafe { SetStdHandle(STD_ERROR_HANDLE, HANDLE(file.into_raw_handle())) };
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::fd::IntoRawFd;
        // SAFETY: dup2 onto fd 2; the original fd is leaked and stays open for the life of the process.
        unsafe { libc::dup2(file.into_raw_fd(), 2) };
    }
}

pub fn logs_dir() -> Option<std::path::PathBuf> {
    let dir = crate::startup::data_dir()?.join("logs");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// "Abrir pasta de logs" in the tray.
pub fn open_logs() {
    let Some(dir) = logs_dir() else { return };
    #[cfg(windows)]
    let opener = "explorer";
    #[cfg(not(windows))]
    let opener = "open";
    let _ = std::process::Command::new(opener).arg(dir).spawn();
}

/// A line in the local log and in Sentry's logs.
pub fn info(line: &str) {
    stamp("INFO", line);
    sentry::logger_info!("{}", line);
}

pub fn warn(line: &str) {
    stamp("WARN", line);
    sentry::logger_warn!("{}", line);
}

fn stamp(level: &str, line: &str) {
    let now = time::OffsetDateTime::now_utc();
    let now = now.format(&time::format_description::well_known::Rfc3339).unwrap_or_default();
    eprintln!("{now} {level} {line}");
}

/// "Algo deu errado?": the user's words as an Info event, with the last 64 KB of screenshare.log.
/// False when this build reports nothing (no DSN).
pub fn send_feedback(text: &str) -> bool {
    if !enabled() {
        return false;
    }
    let log = logs_dir().and_then(|dir| std::fs::read(dir.join("screenshare.log")).ok()).map(|bytes| bytes[bytes.len().saturating_sub(64 * 1024)..].to_vec());
    let text: String = text.chars().take(2_000).collect();
    sentry::with_scope(
        |scope| {
            scope.set_tag("feedback", "user");
            if let Some(buffer) = log {
                scope.add_attachment(sentry::protocol::Attachment { buffer, filename: "screenshare.log".into(), content_type: Some("text/plain".into()), ty: None });
            }
        },
        || sentry::capture_message(&format!("Feedback: {text}"), sentry::Level::Info),
    );
    true
}

/// Whether this build reports to Sentry at all (a release with a DSN).
pub fn enabled() -> bool {
    sentry::Hub::current().client().is_some_and(|client| client.is_enabled())
}
