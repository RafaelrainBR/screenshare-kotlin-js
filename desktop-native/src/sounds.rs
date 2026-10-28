//! The room's tones, as the web's ui/RoomSounds.kt: the same notes, synthesized into a WAV once.
use std::sync::OnceLock;

use desktop_native::session::RoomSound;

const RATE: u32 = 44_100;
const NOTE_SECONDS: f64 = 0.09;

fn notes(sound: RoomSound) -> &'static [f64] {
    match sound {
        RoomSound::Join => &[659.25, 880.0],
        RoomSound::Leave => &[880.0, 659.25],
        RoomSound::Share => &[784.0, 1046.5],
    }
}

/// 16-bit mono PCM, each note with a 10 ms attack and an exponential decay: no click, soft.
pub fn wav(sound: RoomSound) -> Vec<u8> {
    let per_note = (f64::from(RATE) * NOTE_SECONDS) as usize;
    let samples: Vec<i16> = notes(sound)
        .iter()
        .flat_map(|frequency| {
            (0..per_note).map(move |i| {
                let t = i as f64 / f64::from(RATE);
                let envelope = (t / 0.01).min(1.0) * (-t * 40.0).exp();
                (0.2 * envelope * (std::f64::consts::TAU * frequency * t).sin() * f64::from(i16::MAX)) as i16
            })
        })
        .collect();
    let data = (samples.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    wav
}

fn cached(sound: RoomSound) -> &'static [u8] {
    static JOIN: OnceLock<Vec<u8>> = OnceLock::new();
    static LEAVE: OnceLock<Vec<u8>> = OnceLock::new();
    static SHARE: OnceLock<Vec<u8>> = OnceLock::new();
    let cell = match sound {
        RoomSound::Join => &JOIN,
        RoomSound::Leave => &LEAVE,
        RoomSound::Share => &SHARE,
    };
    cell.get_or_init(|| wav(sound))
}

/// Plays without blocking, on the room's chosen output (an endpoint id) when there is one.
/// shortcut: macOS always plays on the system default; route it to the chosen device if asked.
pub fn play(sound: RoomSound, device: Option<&str>) {
    let wav = cached(sound);
    #[cfg(windows)]
    {
        use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
        let device = device.map(str::to_owned);
        std::thread::spawn(move || {
            if device.as_deref().is_some_and(|id| play_on(id, &wav[44..]).is_ok()) {
                return;
            }
            // SAFETY: SND_MEMORY reads the WAV from this pointer while it plays; it is 'static.
            unsafe {
                let _ = PlaySoundW(windows::core::PCWSTR(wav.as_ptr().cast()), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
            }
        });
    }
    #[cfg(not(windows))]
    let _ = device;
    #[cfg(target_os = "macos")]
    {
        let path = std::env::temp_dir().join(format!("screenshare-{sound:?}.wav"));
        if std::fs::metadata(&path).map(|m| m.len()).ok() != Some(wav.len() as u64) {
            let _ = std::fs::write(&path, wav);
        }
        let _ = std::process::Command::new("afplay").arg(&path).spawn();
    }
}

/// Plays 16-bit mono PCM at RATE on one endpoint through WASAPI, blocking until it is heard.
#[cfg(windows)]
fn play_on(device: &str, pcm: &[u8]) -> windows::core::Result<()> {
    use windows::Win32::{
        Media::Audio::{
            AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, IAudioClient, IAudioRenderClient, IMMDeviceEnumerator,
            MMDeviceEnumerator, WAVE_FORMAT_PCM, WAVEFORMATEX,
        },
        System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx},
    };
    let format = WAVEFORMATEX { wFormatTag: WAVE_FORMAT_PCM as u16, nChannels: 1, nSamplesPerSec: RATE, nAvgBytesPerSec: RATE * 2, nBlockAlign: 2, wBitsPerSample: 16, cbSize: 0 };
    // SAFETY: COM calls on this thread; the buffer WASAPI hands out holds the frames asked for, in `format`.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let client: IAudioClient = devices.GetDevice(&windows::core::HSTRING::from(device))?.Activate(CLSCTX_ALL, None)?;
        client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, 2_000_000, 0, &format, None)?;
        let render: IAudioRenderClient = client.GetService()?;
        let size = client.GetBufferSize()? as usize;
        let mut rest = pcm;
        client.Start()?;
        while !rest.is_empty() {
            let frames = (size - client.GetCurrentPadding()? as usize).min(rest.len() / 2);
            let buffer = render.GetBuffer(frames as u32)?;
            std::ptr::copy_nonoverlapping(rest.as_ptr(), buffer, frames * 2);
            render.ReleaseBuffer(frames as u32, 0)?;
            rest = &rest[frames * 2..];
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        while client.GetCurrentPadding()? > 0 {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        client.Stop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_sound_is_a_short_valid_wav() {
        for sound in [RoomSound::Join, RoomSound::Leave, RoomSound::Share] {
            let wav = wav(sound);
            assert_eq!((&wav[0..4], &wav[8..12], &wav[36..40]), (&b"RIFF"[..], &b"WAVE"[..], &b"data"[..]));
            let data = u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize;
            assert_eq!(wav.len(), 44 + data);
            // Two notes of 90 ms at 44.1 kHz, 16-bit.
            assert_eq!(data, 2 * 3_969 * 2);
            assert!(wav[44..].chunks(2).any(|s| i16::from_le_bytes([s[0], s[1]]).unsigned_abs() > 1_000));
        }
    }

    /// Makes noise: the system plays our WAV from memory.
    #[cfg(windows)]
    #[test]
    #[ignore = "plays a tone on the default output"]
    fn windows_plays_the_wav_from_memory() {
        use windows::Win32::Media::Audio::{PlaySoundW, SND_MEMORY, SND_NODEFAULT, SND_SYNC};
        let wav = cached(RoomSound::Join);
        // SAFETY: SND_SYNC returns after playing, while `wav` is alive ('static anyway).
        let played = unsafe { PlaySoundW(windows::core::PCWSTR(wav.as_ptr().cast()), None, SND_MEMORY | SND_SYNC | SND_NODEFAULT) };
        assert!(played.as_bool());
    }

    /// Makes noise: the same tone through WASAPI on the default endpoint, picked by its id.
    #[cfg(windows)]
    #[test]
    #[ignore = "plays a tone on the default output"]
    fn windows_plays_on_a_device_by_id() {
        use windows::Win32::{
            Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender},
            System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx},
        };
        // SAFETY: COM on this thread; GetId's string is ours to read.
        let id = unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).unwrap();
            devices.GetDefaultAudioEndpoint(eRender, eConsole).unwrap().GetId().unwrap().to_string().unwrap()
        };
        play_on(&id, &cached(RoomSound::Join)[44..]).unwrap();
    }
}
