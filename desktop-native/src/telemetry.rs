//! Sentry (docs/native-desktop-plan.md, F7): panics, errors and logs, only from builds given a DSN.
//! Events carry no name, room or machine: the room and the username stay out of every message.
use std::sync::Arc;
use std::time::{Duration, Instant};

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

/// A breadcrumb in this process and in the crash reporter's own copy, so a C++ crash (a minidump,
/// sent by the other process) still comes with it.
pub fn breadcrumb(category: &str, message: &str) {
    let breadcrumb = sentry::Breadcrumb { category: Some(category.into()), message: Some(message.into()), ..Default::default() };
    sentry::with_integration(|minidump: &sentry::integrations::minidump::MinidumpIntegration, _| minidump.add_breadcrumb(breadcrumb.clone()));
    sentry::add_breadcrumb(breadcrumb);
}

const MB: u64 = 1 << 20;
const MEMORY_EVERY: Duration = Duration::from_secs(30 * 60);
const MEMORY_GROWTH: u64 = 256 * MB;

/// A "memory" breadcrumb every 30 minutes, and sooner when the process grew by 256 MB: a crash on a
/// failed allocation (libwebrtc's AlignedMalloc) then shows a leak apart from a PC out of memory.
/// Few on purpose, so they don't push the "ui" steps out of the breadcrumb buffer.
pub fn watch_memory() {
    if !enabled() {
        return;
    }
    std::thread::spawn(|| {
        let mut last: Option<(u64, Instant)> = None;
        loop {
            std::thread::sleep(Duration::from_secs(60));
            let Some(memory) = memory() else { continue };
            if !memory_due(last.map(|(bytes, at)| (bytes, at.elapsed())), memory.private) {
                continue;
            }
            last = Some((memory.private, Instant::now()));
            breadcrumb("memory", &memory.describe());
        }
    });
}

fn memory_due(last: Option<(u64, Duration)>, private: u64) -> bool {
    last.is_none_or(|(bytes, elapsed)| elapsed >= MEMORY_EVERY || private >= bytes + MEMORY_GROWTH)
}

#[cfg_attr(not(windows), allow(dead_code))]
struct Memory {
    /// The process's committed private bytes: what a leak grows.
    private: u64,
    /// Windows fails an allocation when the commit limit (RAM + page file) is used up, not the RAM.
    commit_free: u64,
    commit_total: u64,
    ram_free: u64,
    ram_total: u64,
}

impl Memory {
    fn describe(&self) -> String {
        format!(
            "process {} MB private; commit {} of {} MB free; RAM {} of {} MB free",
            self.private / MB,
            self.commit_free / MB,
            self.commit_total / MB,
            self.ram_free / MB,
            self.ram_total / MB
        )
    }
}

#[cfg(windows)]
fn memory() -> Option<Memory> {
    use windows::Win32::System::{
        ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX},
        SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
        Threading::GetCurrentProcess,
    };
    let mut process = PROCESS_MEMORY_COUNTERS_EX { cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32, ..Default::default() };
    // SAFETY: the counters struct outlives the call and cb is its real size.
    unsafe { GetProcessMemoryInfo(GetCurrentProcess(), (&raw mut process).cast::<PROCESS_MEMORY_COUNTERS>(), process.cb) }.ok()?;
    let mut system = MEMORYSTATUSEX { dwLength: size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    // SAFETY: as above, dwLength is the struct's size.
    unsafe { GlobalMemoryStatusEx(&mut system) }.ok()?;
    Some(Memory {
        private: process.PrivateUsage as u64,
        commit_free: system.ullAvailPageFile,
        commit_total: system.ullTotalPageFile,
        ram_free: system.ullAvailPhys,
        ram_total: system.ullTotalPhys,
    })
}

// shortcut: Windows only; add task_info on macOS when a Mac crash needs it.
#[cfg(not(windows))]
fn memory() -> Option<Memory> {
    None
}

/// Whether this build reports to Sentry at all (a release with a DSN).
pub fn enabled() -> bool {
    sentry::Hub::current().client().is_some_and(|client| client.is_enabled())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_breadcrumbs_are_periodic_or_on_growth() {
        assert!(memory_due(None, 0), "the first sample is always logged");
        assert!(!memory_due(Some((500 * MB, Duration::from_secs(60))), 600 * MB));
        assert!(memory_due(Some((500 * MB, Duration::from_secs(60))), 756 * MB));
        assert!(memory_due(Some((500 * MB, MEMORY_EVERY)), 400 * MB));
    }

    #[cfg(windows)]
    #[test]
    fn reads_this_process_memory() {
        let memory = memory().expect("memory counters");
        assert!(memory.private > 0 && memory.ram_total > 0 && memory.commit_total >= memory.ram_total / 2);
        assert!(memory.describe().starts_with("process "));
    }
}
