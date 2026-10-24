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
    options.before_send = Some(Arc::new(|mut event| {
        // The PC's name and anything user-like never leave the machine.
        event.server_name = None;
        event.user = None;
        Some(event)
    }));
    let guard = sentry::init((dsn, options));
    guard.is_enabled().then_some(guard)
}
