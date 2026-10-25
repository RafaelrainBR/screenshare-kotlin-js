//! "Por que está travando?", as the web's services/ConnectionQuality.kt: what holds a screen's
//! quality back, as far as this side measures. The sharer sees why its encoder backed off; a viewer
//! sees loss and its own skipped frames. Same thresholds and texts as the web.
use libwebrtc::stats::RtcStats;
use serde_json::{Value, json};

/// How often the session asks for `screenQuality`.
pub const WINDOW_MS: u64 = 3_000;

// Limited for half the window; NACK hides up to a few percent of loss; a static screen sends few packets.
const LIMITED_SHARE: f64 = 0.5;
const LOSS_SHARE: f64 = 0.05;
const MIN_PACKETS: f64 = 60.0;
const DROPPED_SHARE: f64 = 0.15;
const MIN_FRAMES: f64 = 30.0;

/// In priority order: the sharer's own report (send-share-quality) explains its screen first, and
/// the CPU holds every viewer back, so it explains more than one slow link.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Problem {
    PeerCpu,
    PeerBandwidth,
    SenderCpu,
    SenderBandwidth,
    UnstableConnection,
    ViewerDecode,
}

impl Problem {
    pub fn label(self, peer: &str) -> String {
        match self {
            Self::PeerCpu => format!("PC de {peer} no limite"),
            Self::PeerBandwidth => format!("Internet de {peer} no limite"),
            Self::SenderCpu | Self::ViewerDecode => "Seu PC no limite".into(),
            Self::SenderBandwidth => "Sua internet no limite".into(),
            Self::UnstableConnection => "Conexão instável".into(),
        }
    }

    /// What our shown problem tells the room; only the encoder's limits are the sharer's to report.
    pub fn share_problem(self) -> Option<&'static str> {
        match self {
            Self::SenderCpu => Some("cpu"),
            Self::SenderBandwidth => Some("bandwidth"),
            _ => None,
        }
    }

    /// A sharer's report; a value this version does not know shows nothing.
    pub fn from_peer(problem: &str) -> Option<Self> {
        match problem {
            "cpu" => Some(Self::PeerCpu),
            "bandwidth" => Some(Self::PeerBandwidth),
            _ => None,
        }
    }

    pub fn explanation(self, peer: &str, viewers: usize) -> String {
        match self {
            Self::PeerCpu => format!("O PC de {peer} não está dando conta de codificar o vídeo, então a qualidade da tela caiu. Não é a sua conexão."),
            Self::PeerBandwidth => format!("A internet de {peer} não está dando conta de enviar nessa qualidade, então a qualidade da tela caiu."),
            Self::SenderCpu => "Seu PC não está dando conta de codificar o vídeo, então a qualidade da sua tela caiu. Feche programas pesados ou escolha um preset mais leve.".into(),
            Self::SenderBandwidth => format!(
                "Sua internet não está dando conta de enviar nessa qualidade, então a qualidade da sua tela caiu. Escolha um preset mais leve.{}",
                if viewers > 1 { " Cada pessoa assistindo usa mais da sua internet." } else { "" }
            ),
            Self::UnstableConnection => format!("A conexão entre você e {peer} está perdendo dados, por isso o vídeo trava. Pode ser a sua internet ou a de {peer}."),
            Self::ViewerDecode => "Seu PC não está dando conta de mostrar o vídeo e está pulando quadros. Feche programas pesados.".into(),
        }
    }
}

/// Cumulative outbound counters: seconds the encoder was limited by the CPU and by bandwidth.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SendSample {
    pub at_ms: f64,
    pub cpu_seconds: f64,
    pub bandwidth_seconds: f64,
}

/// Cumulative inbound video counters.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReceiveSample {
    pub packets_received: f64,
    pub packets_lost: f64,
    pub frames_decoded: f64,
    pub frames_dropped: f64,
}

pub fn send_problem(previous: SendSample, current: SendSample) -> Option<Problem> {
    let elapsed = (current.at_ms - previous.at_ms) / 1000.0;
    if elapsed <= 0.0 {
        return None;
    }
    if (current.cpu_seconds - previous.cpu_seconds) / elapsed >= LIMITED_SHARE {
        Some(Problem::SenderCpu)
    } else if (current.bandwidth_seconds - previous.bandwidth_seconds) / elapsed >= LIMITED_SHARE {
        Some(Problem::SenderBandwidth)
    } else {
        None
    }
}

pub fn receive_problem(previous: ReceiveSample, current: ReceiveSample) -> Option<Problem> {
    let received = current.packets_received - previous.packets_received;
    // Duplicates make packetsLost go down.
    let lost = (current.packets_lost - previous.packets_lost).max(0.0);
    let decoded = current.frames_decoded - previous.frames_decoded;
    let dropped = current.frames_dropped - previous.frames_dropped;
    if received + lost >= MIN_PACKETS && lost / (received + lost) >= LOSS_SHARE {
        Some(Problem::UnstableConnection)
    } else if decoded + dropped >= MIN_FRAMES && dropped / (decoded + dropped) >= DROPPED_SHARE {
        Some(Problem::ViewerDecode)
    } else {
        None
    }
}

/// A problem shows after two bad windows in a row and goes after three good ones, so a blip never flashes.
#[derive(Debug, Default)]
pub struct Signal {
    pub shown: Option<Problem>,
    pending: Option<Problem>,
    streak: u8,
}

impl Signal {
    pub fn update(&mut self, window: Option<Problem>) -> Option<Problem> {
        if window == self.shown {
            self.pending = None;
            self.streak = 0;
            return self.shown;
        }
        if window != self.pending {
            self.pending = window;
            self.streak = 0;
        }
        self.streak += 1;
        if self.streak >= if window.is_none() { 3 } else { 2 } {
            self.shown = window;
            self.pending = None;
            self.streak = 0;
        }
        self.shown
    }
}

/// One peer's counters for the `screenQuality` event: our screen's outbound stream, sent on the
/// "screen" track (attach_real_screen), and every inbound video, which the UI sorts by kind.
pub fn samples(stats: &[RtcStats], screen_track: &str) -> Value {
    let source = stats.iter().find_map(|stat| match stat {
        RtcStats::MediaSource(source) if source.source.track_identifier == screen_track => Some(source.rtc.id.clone()),
        _ => None,
    });
    let send = stats.iter().find_map(|stat| match stat {
        RtcStats::OutboundRtp(out) if out.stream.kind == "video" && Some(&out.outbound.media_source_id) == source.as_ref() => {
            let durations = &out.outbound.quality_limitation_durations;
            Some(json!({
                "cpu": durations.get("cpu").copied().unwrap_or(0.0),
                "bandwidth": durations.get("bandwidth").copied().unwrap_or(0.0),
            }))
        }
        _ => None,
    });
    let receive: Vec<Value> = stats
        .iter()
        .filter_map(|stat| match stat {
            RtcStats::InboundRtp(inbound) if inbound.stream.kind == "video" => Some(json!({
                "track": inbound.inbound.track_identifier,
                "packetsReceived": inbound.received.packets_received,
                "packetsLost": inbound.received.packets_lost,
                "framesDecoded": inbound.inbound.frames_decoded,
                "framesDropped": inbound.inbound.frames_dropped,
            })),
            _ => None,
        })
        .collect();
    json!({ "send": send, "receive": receive })
}

pub fn send_sample(value: &Value, at_ms: f64) -> Option<SendSample> {
    Some(SendSample { at_ms, cpu_seconds: value.get("cpu")?.as_f64()?, bandwidth_seconds: value.get("bandwidth")?.as_f64()? })
}

pub fn receive_sample(value: &Value) -> Option<ReceiveSample> {
    let number = |key: &str| value.get(key).and_then(Value::as_f64);
    Some(ReceiveSample {
        packets_received: number("packetsReceived")?,
        packets_lost: number("packetsLost")?,
        frames_decoded: number("framesDecoded")?,
        frames_dropped: number("framesDropped")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn send(at_ms: f64, cpu_seconds: f64, bandwidth_seconds: f64) -> SendSample {
        SendSample { at_ms, cpu_seconds, bandwidth_seconds }
    }

    fn receive(packets_received: f64, packets_lost: f64, frames_decoded: f64, frames_dropped: f64) -> ReceiveSample {
        ReceiveSample { packets_received, packets_lost, frames_decoded, frames_dropped }
    }

    #[test]
    fn the_sharer_sees_what_limited_its_encoder_for_half_the_window() {
        assert_eq!(send_problem(send(0.0, 10.0, 0.0), send(3_000.0, 11.5, 0.0)), Some(Problem::SenderCpu));
        assert_eq!(send_problem(send(0.0, 0.0, 4.0), send(3_000.0, 0.0, 6.0)), Some(Problem::SenderBandwidth));
        assert_eq!(send_problem(send(0.0, 0.0, 0.0), send(3_000.0, 1.0, 1.0)), None);
        assert_eq!(send_problem(send(0.0, 0.0, 0.0), send(3_000.0, 3.0, 3.0)), Some(Problem::SenderCpu));
        assert_eq!(send_problem(send(3_000.0, 0.0, 0.0), send(3_000.0, 9.0, 9.0)), None);
    }

    #[test]
    fn the_viewer_sees_loss_first_and_then_its_own_skipped_frames() {
        let start = receive(1_000.0, 10.0, 500.0, 0.0);
        assert_eq!(receive_problem(start, receive(1_180.0, 30.0, 560.0, 40.0)), Some(Problem::UnstableConnection));
        assert_eq!(receive_problem(start, receive(1_300.0, 11.0, 560.0, 20.0)), Some(Problem::ViewerDecode));
        assert_eq!(receive_problem(start, receive(1_300.0, 12.0, 680.0, 2.0)), None);
        // A static screen sends a handful of packets: one lost is not a trend.
        assert_eq!(receive_problem(start, receive(1_010.0, 12.0, 503.0, 0.0)), None);
        assert_eq!(receive_problem(start, receive(1_300.0, 5.0, 680.0, 0.0)), None);
    }

    #[test]
    fn a_problem_shows_after_two_windows_and_goes_after_three() {
        let mut signal = Signal::default();
        let cpu = Some(Problem::SenderCpu);
        let shown: Vec<_> = [cpu, None, cpu, cpu, None, None, None].into_iter().map(|window| signal.update(window)).collect();
        assert_eq!(shown, [None, None, None, cpu, cpu, cpu, None]);
    }

    #[test]
    fn explanations_name_the_peer_and_mention_each_viewer() {
        let unstable = Problem::UnstableConnection.explanation("Ana", 0);
        assert!(unstable.contains("Ana") && unstable.contains("sua internet"));
        assert!(Problem::SenderBandwidth.explanation("", 2).contains("Cada pessoa"));
        assert!(!Problem::SenderBandwidth.explanation("", 1).contains("Cada pessoa"));
    }

    #[test]
    fn only_the_encoder_limits_go_to_the_room_and_come_back_named() {
        assert_eq!(Problem::SenderCpu.share_problem(), Some("cpu"));
        assert_eq!(Problem::SenderBandwidth.share_problem(), Some("bandwidth"));
        assert_eq!(Problem::UnstableConnection.share_problem(), None);
        assert_eq!(Problem::from_peer("cpu"), Some(Problem::PeerCpu));
        assert_eq!(Problem::from_peer("bandwidth"), Some(Problem::PeerBandwidth));
        assert_eq!(Problem::from_peer("gpu"), None);
        assert_eq!(Problem::PeerCpu.label("Ana"), "PC de Ana no limite");
        assert!(Problem::PeerCpu.explanation("Ana", 0).contains("Não é a sua conexão"));
    }

    #[test]
    fn event_samples_round_trip() {
        let value = json!({ "cpu": 1.5, "bandwidth": 0.0 });
        assert_eq!(send_sample(&value, 9.0), Some(send(9.0, 1.5, 0.0)));
        let value = json!({ "track": "t", "packetsReceived": 10, "packetsLost": -1, "framesDecoded": 3, "framesDropped": 0 });
        assert_eq!(receive_sample(&value), Some(receive(10.0, -1.0, 3.0, 0.0)));
        assert_eq!(receive_sample(&json!({})), None);
    }
}
