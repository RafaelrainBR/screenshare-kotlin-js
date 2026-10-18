//! Remote voices for the speaking ring and "Silenciar áudio para você". A voice is a remote audio
//! track whose stream has no video, as in the web's onTrack (handleRemoteAudio); a shared screen's
//! audio travels with its video and is left alone.
use std::collections::{HashMap, HashSet};

use libwebrtc::{audio_track::RtcAudioTrack, stats::RtcStats};

/// The local microphone track id (`create_device_audio_track("microphone")`).
pub const MICROPHONE: &str = "microphone";

#[derive(Default)]
pub struct Voices {
    tracks: HashMap<String, Vec<RtcAudioTrack>>,
    muted: HashSet<String>,
    /// "Volume para você", 0..1, per peer: the voice, and the audio that travels with a shared screen.
    volumes: HashMap<String, f64>,
    screens: HashMap<String, Vec<RtcAudioTrack>>,
    screen_volumes: HashMap<String, f64>,
}

/// 0 disables the track (libwebrtc then plays silence); above, the remote source's playout gain.
fn apply(track: &RtcAudioTrack, volume: f64) {
    track.set_enabled(volume > 0.0);
    if volume > 0.0 {
        track.set_volume(volume);
    }
}

fn keep(tracks: &mut Vec<RtcAudioTrack>, track: RtcAudioTrack) {
    if !tracks.iter().any(|known| known.id() == track.id()) {
        tracks.push(track);
    }
}

impl Voices {
    pub fn add(&mut self, peer: &str, track: RtcAudioTrack) {
        // A voice arriving after "Silenciar" or a volume change (renegotiation, mic turned on later) starts like the others.
        if self.muted.contains(peer) {
            track.set_enabled(false);
        } else if let Some(volume) = self.volumes.get(peer) {
            apply(&track, *volume);
        }
        keep(self.tracks.entry(peer.to_owned()).or_default(), track);
    }

    /// The audio of a shared screen: not a voice, but it has its own volume on the screen tile.
    pub fn add_screen(&mut self, peer: &str, track: RtcAudioTrack) {
        if let Some(volume) = self.screen_volumes.get(peer) {
            apply(&track, *volume);
        }
        keep(self.screens.entry(peer.to_owned()).or_default(), track);
    }

    pub fn remove(&mut self, peer: &str) {
        self.tracks.remove(peer);
        self.muted.remove(peer);
        self.volumes.remove(peer);
        self.screens.remove(peer);
        self.screen_volumes.remove(peer);
    }

    pub fn set_volume(&mut self, peer: &str, volume: f64, screen: bool) {
        let (tracks, volumes) = if screen { (&self.screens, &mut self.screen_volumes) } else { (&self.tracks, &mut self.volumes) };
        volumes.insert(peer.to_owned(), volume);
        if !screen {
            self.muted.remove(peer);
        }
        for track in tracks.get(peer).into_iter().flatten() {
            apply(track, volume);
        }
    }

    /// A disabled remote audio track plays silence: libwebrtc sets its output volume to 0.
    pub fn set_muted(&mut self, peer: &str, muted: bool) {
        if muted {
            self.muted.insert(peer.to_owned());
        } else {
            self.muted.remove(peer);
        }
        for track in self.tracks.get(peer).into_iter().flatten() {
            track.set_enabled(!muted);
        }
    }

    pub fn ids(&self, peer: &str) -> HashSet<String> {
        self.tracks.get(peer).into_iter().flatten().map(RtcAudioTrack::id).collect()
    }
}

/// (loudest voice of the peer, local microphone), 0..1 as the WebRTC `audioLevel`.
pub fn levels(stats: &[RtcStats], voices: &HashSet<String>) -> (f64, f64) {
    stats.iter().fold((0.0, 0.0), |(remote, local), stat| match stat {
        RtcStats::InboundRtp(value) if value.stream.kind == "audio" && voices.contains(&value.inbound.track_identifier) => {
            (remote.max(value.inbound.audio_level), local)
        }
        RtcStats::MediaSource(value) if value.source.kind == "audio" && value.source.track_identifier == MICROPHONE => {
            (remote, local.max(value.audio.audio_level))
        }
        _ => (remote, local),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use libwebrtc::stats::{InboundRtpStats, MediaSourceStats};

    fn inbound(track: &str, kind: &str, level: f64) -> RtcStats {
        let mut stat = InboundRtpStats::default();
        stat.stream.kind = kind.into();
        stat.inbound.track_identifier = track.into();
        stat.inbound.audio_level = level;
        RtcStats::InboundRtp(stat)
    }

    fn source(track: &str, level: f64) -> RtcStats {
        let mut stat = MediaSourceStats::default();
        stat.source.kind = "audio".into();
        stat.source.track_identifier = track.into();
        stat.audio.audio_level = level;
        RtcStats::MediaSource(stat)
    }

    #[test]
    fn only_voices_and_the_microphone_count() {
        let voices = HashSet::from(["voice".to_owned()]);
        let stats = [
            inbound("voice", "audio", 0.2),
            inbound("screen-audio", "audio", 0.9),
            inbound("voice", "video", 0.8),
            source(MICROPHONE, 0.3),
            source("screen-audio", 0.7),
        ];
        assert_eq!(levels(&stats, &voices), (0.2, 0.3), "screen audio and video never count as speaking");
        assert_eq!(levels(&[], &voices), (0.0, 0.0));
    }
}
