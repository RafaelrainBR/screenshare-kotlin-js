//! Anonymous usage metrics, as the web's platform.metric (the "Métricas do Screenshare" design):
//! a name and a few buckets as a Sentry log, never a name, room, address or window title. Only
//! builds with a DSN send; "Enviar estatísticas de uso anônimas" turns them off, errors stay on.
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime},
};

static ON: AtomicBool = AtomicBool::new(true);

pub fn set_enabled(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

pub fn send(name: &str, attributes: &[(&str, String)]) {
    if !ON.load(Ordering::Relaxed) {
        return;
    }
    let platform = if cfg!(target_os = "macos") { "native-macos" } else { "native-windows" };
    let mut fields = sentry::protocol::Map::new();
    for (key, value) in std::iter::once(("platform", platform.to_owned())).chain(attributes.iter().map(|(key, value)| (*key, value.clone()))) {
        fields.insert(key.to_owned(), sentry::protocol::LogAttribute(sentry::protocol::Value::from(value)));
    }
    // Without a client (local builds, no DSN) the hub drops it.
    sentry::Hub::current().capture_log(sentry::protocol::Log {
        level: sentry::protocol::LogLevel::Info,
        body: name.to_owned(),
        trace_id: None,
        timestamp: SystemTime::now(),
        severity_number: None,
        attributes: fields,
    });
}

/// Buckets, not exact values: they answer the questions without making anyone recognizable.
pub fn people_bucket(count: usize) -> &'static str {
    match count {
        0 | 1 => "1",
        2 => "2",
        3..=5 => "3-5",
        _ => "6+",
    }
}

pub fn minutes_bucket(elapsed: Duration) -> &'static str {
    match elapsed.as_secs() {
        0..60 => "<1",
        60..300 => "1-5",
        300..900 => "5-15",
        900..3600 => "15-60",
        _ => "60+",
    }
}

pub fn seconds_bucket(elapsed: Duration) -> &'static str {
    match elapsed.as_secs() {
        0..5 => "<5",
        5..30 => "5-30",
        _ => "30+",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_carry_buckets_not_exact_values() {
        assert_eq!([1, 2, 3, 5, 6].map(people_bucket), ["1", "2", "3-5", "3-5", "6+"]);
        assert_eq!([59, 60, 300, 900, 3600].map(|s| minutes_bucket(Duration::from_secs(s))), ["<1", "1-5", "5-15", "15-60", "60+"]);
        assert_eq!([4, 5, 30].map(|s| seconds_bucket(Duration::from_secs(s))), ["<5", "5-30", "30+"]);
    }
}
