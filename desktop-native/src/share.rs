//! Share encoder settings ported 1:1 from the web client: SharePreset.kt, the codec order in
//! RTCPeerConnectionDecorator.preferVideoCodecs and AudioQuality.upgradeAudioQualitySdp.
//! Same numbers and rules; change both sides together.

use libwebrtc::rtp_parameters::{DegradationPreference, RtpCodecCapability};
use serde::{Deserialize, Serialize};

/// What the encoder gives up when the network tightens (SharePriority.kt).
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SharePriority {
    Sharpness,
    Balanced,
    Smoothness,
}

impl SharePriority {
    /// The web client's contentHint: "text" turns on the codec's screen-content tools.
    /// libwebrtc's equivalent is a screencast source, fixed for the whole share.
    pub fn screencast(self) -> bool {
        self == Self::Sharpness
    }

    pub fn degradation(self) -> DegradationPreference {
        match self {
            Self::Sharpness => DegradationPreference::MaintainResolution,
            Self::Balanced => DegradationPreference::Balanced,
            Self::Smoothness => DegradationPreference::MaintainFramerate,
        }
    }
}

/// `max_bitrate: None` means automatic, from the share height.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShareProfile {
    pub fps: u32,
    pub priority: SharePriority,
    #[serde(default)]
    pub max_bitrate: Option<u64>,
}

/// Starting points; the advanced settings edit the profile they fill in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharePreset {
    Game,
    Video,
    Text,
}

impl SharePreset {
    pub const ALL: [Self; 3] = [Self::Game, Self::Video, Self::Text];

    pub fn profile(self) -> ShareProfile {
        let (fps, priority) = match self {
            Self::Game => (60, SharePriority::Smoothness),
            Self::Video => (30, SharePriority::Balanced),
            // A still screen costs almost nothing at any fps; 10 keeps scrolling and the cursor readable.
            Self::Text => (10, SharePriority::Sharpness),
        };
        ShareProfile { fps, priority, max_bitrate: None }
    }
}

pub fn share_bitrate(profile: ShareProfile, height: u32) -> u64 {
    if let Some(bitrate) = profile.max_bitrate {
        return bitrate;
    }
    // Only a ceiling: a still screen of text stays far below it.
    let base: u64 = match height {
        ..=720 => 2_500_000,
        ..=1080 => 6_000_000,
        ..=1440 => 10_000_000,
        _ => 15_000_000,
    };
    if profile.fps >= 60 { (base as f64 * 1.5) as u64 } else { base }
}

/// VP9 > H264 > VP8 > AV1, keeping RTX/RED/ULPFEC: without them fullscreen video corrupts
/// as soon as packets are lost.
pub fn preferred_video_codecs(capabilities: Vec<RtpCodecCapability>) -> Vec<RtpCodecCapability> {
    let ends = |codec: &RtpCodecCapability, id: &str| codec.mime_type.to_lowercase().ends_with(id);
    let mut preferred = Vec::new();
    for id in ["vp9", "h264", "vp8", "av1", "rtx", "red", "ulpfec"] {
        preferred.extend(capabilities.iter().filter(|codec| ends(codec, id)).cloned());
    }
    preferred
}

const OPUS_EXTRA: &str = "stereo=1;sprop-stereo=1;maxaveragebitrate=192000;maxplaybackrate=48000;ptime=20;minptime=10;useinbandfec=1;usedtx=1";
const OPUS_REPLACED: [&str; 8] = [
    "stereo",
    "sprop-stereo",
    "maxaveragebitrate",
    "maxplaybackrate",
    "ptime",
    "minptime",
    "usedtx",
    "useinbandfec",
];

/// Stereo, 48 kHz, 192 kbps ceiling, 20 ms packets, in-band FEC and DTX on every Opus fmtp line,
/// so shared movies and music keep their full spectrum and survive packet loss.
pub fn upgrade_audio_quality_sdp(sdp: &str) -> String {
    let opus: Vec<String> = sdp
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("a=rtpmap:")?;
            let (payload, codec) = rest.split_once(' ')?;
            codec
                .to_ascii_lowercase()
                .starts_with("opus/48000")
                .then(|| payload.to_owned())
        })
        .collect();
    if opus.is_empty() {
        return sdp.to_owned();
    }
    sdp.split_inclusive('\n')
        .map(|raw| {
            let line = raw.trim_end_matches(['\r', '\n']);
            let ending = &raw[line.len()..];
            let Some((payload, params)) = line
                .strip_prefix("a=fmtp:")
                .and_then(|rest| rest.split_once(' '))
            else {
                return raw.to_owned();
            };
            if !opus.iter().any(|pt| pt == payload) {
                return raw.to_owned();
            }
            let keep: Vec<&str> = params
                .split(';')
                .filter(|param| {
                    let key = param.split('=').next().unwrap_or("").to_ascii_lowercase();
                    !(param.contains('=') && OPUS_REPLACED.contains(&key.as_str()))
                })
                .collect();
            let keep = keep.join(";");
            let joined = if keep.is_empty() { OPUS_EXTRA.to_owned() } else { format!("{OPUS_EXTRA};{keep}") };
            format!("a=fmtp:{payload} {}{ending}", joined.trim_end_matches(';'))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_and_bitrates_match_share_preset_kt() {
        let table: Vec<_> = SharePreset::ALL.iter().map(|p| (p.profile().fps, p.profile().priority)).collect();
        assert_eq!(
            table,
            [(60, SharePriority::Smoothness), (30, SharePriority::Balanced), (10, SharePriority::Sharpness)]
        );
        let video = SharePreset::Video.profile();
        let game = SharePreset::Game.profile();
        for (height, base) in [(480, 2_500_000), (720, 2_500_000), (1080, 6_000_000), (1440, 10_000_000), (2160, 15_000_000)] {
            assert_eq!(share_bitrate(video, height), base);
            assert_eq!(share_bitrate(game, height), base * 3 / 2);
        }
        assert_eq!(share_bitrate(ShareProfile { max_bitrate: Some(1_234), ..game }, 2160), 1_234);
        assert_eq!(SharePriority::Sharpness.degradation(), DegradationPreference::MaintainResolution);
        assert_eq!(SharePriority::Smoothness.degradation(), DegradationPreference::MaintainFramerate);
        assert!(SharePriority::Sharpness.screencast() && !SharePriority::Balanced.screencast());
        let parsed: ShareProfile = serde_json::from_str(r#"{"fps":30,"priority":"SHARPNESS"}"#).unwrap();
        assert_eq!(parsed, ShareProfile { fps: 30, priority: SharePriority::Sharpness, max_bitrate: None });
    }

    #[test]
    fn codec_order_is_vp9_h264_vp8_av1_and_keeps_repair_codecs() {
        let caps = ["video/VP8", "video/AV1", "video/H264", "video/flexfec-03", "video/VP9", "video/rtx", "video/red", "video/ulpfec"]
            .map(|mime| RtpCodecCapability { channels: None, clock_rate: Some(90_000), mime_type: mime.into(), sdp_fmtp_line: None });
        let order: Vec<_> = preferred_video_codecs(caps.to_vec()).into_iter().map(|c| c.mime_type).collect();
        assert_eq!(order, ["video/VP9", "video/H264", "video/VP8", "video/AV1", "video/rtx", "video/red", "video/ulpfec"]);
    }

    #[test]
    fn opus_fmtp_is_upgraded_and_other_lines_stay_byte_identical() {
        let sdp = "v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111 63\r\na=rtpmap:111 opus/48000/2\r\na=fmtp:111 minptime=10;useinbandfec=1;stereo=0\r\na=rtpmap:63 red/48000/2\r\na=fmtp:63 111/111\r\nm=video 9 UDP/TLS/RTP/SAVPF 96\r\na=fmtp:96 profile-id=0\r\n";
        let out = upgrade_audio_quality_sdp(sdp);
        assert!(out.contains(&format!("a=fmtp:111 {OPUS_EXTRA}\r\n")), "{out}");
        assert!(out.contains("a=fmtp:63 111/111\r\n") && out.contains("a=fmtp:96 profile-id=0\r\n"));
        let kept = upgrade_audio_quality_sdp("a=rtpmap:109 OPUS/48000/2\na=fmtp:109 cbr=1;ptime=40\n");
        assert_eq!(kept, format!("a=rtpmap:109 OPUS/48000/2\na=fmtp:109 {OPUS_EXTRA};cbr=1\n"));
        assert_eq!(upgrade_audio_quality_sdp("m=video 9\r\n"), "m=video 9\r\n");
    }
}
