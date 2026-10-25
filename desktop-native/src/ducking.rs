//! Windows lowers every other app ("Comunicações" in the Sound settings) while a stream plays on
//! the communications role, and libwebrtc's voice engine picks kDefaultCommunicationDevice when
//! the first peer connection is made. The fork's AdmProxy plays on kDefaultDevice instead (the same
//! speakers, following the system default). IAudioSessionControl2::SetDuckingPreference, which
//! 0.2.x tried, does not stop this ducking: measured below, it made no difference.

/// Another process plays a quiet tone; the speakers' mix is measured while a peer's audio plays here
/// through the platform ADM, as in a room. On Windows 10 with the fork before the fix, the tone fell
/// from 0.050 to 0.006 (an 87% cut, also with the microphone on); with it, it stays at 0.050.
#[test]
#[ignore = "plays a quiet tone on the speakers"]
fn a_room_does_not_lower_other_apps() {
    use std::time::{Duration, Instant};

    use libwebrtc::{
        audio_frame::AudioFrame,
        audio_source::{AudioSourceOptions, native::NativeAudioSource},
        ice_candidate::IceCandidate,
        peer_connection::OfferOptions,
        peer_connection_factory::{PeerConnectionFactory, RtcConfiguration, native::PeerConnectionFactoryExt},
    };
    use windows::Win32::{
        Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender},
        System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx},
    };

    // 3 s of a 1873 Hz tone (rare enough in music to pick out of the mix) at 5% amplitude, looped by another process.
    let samples: Vec<u8> = (0..3 * 44_100)
        .flat_map(|i| ((0.05 * (std::f64::consts::TAU * 1_873.0 * f64::from(i) / 44_100.0).sin() * 32_767.0) as i16).to_le_bytes())
        .collect();
    let mut wav = b"RIFF".to_vec();
    wav.extend_from_slice(&(36 + samples.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    for field in [16u32.to_le_bytes(), [1, 0, 1, 0], 44_100u32.to_le_bytes(), 88_200u32.to_le_bytes(), [2, 0, 16, 0]] {
        wav.extend_from_slice(&field);
    }
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    wav.extend_from_slice(&samples);
    let path = std::env::temp_dir().join("screenshare-ducking-tone.wav");
    std::fs::write(&path, wav).unwrap();
    let mut other_app = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &format!("$p = New-Object Media.SoundPlayer '{}'; $p.PlayLooping(); Start-Sleep 30", path.display())])
        .spawn()
        .unwrap();

    // The speakers' own mix (an endpoint loopback, which ducking shows in), reduced to the tone's
    // 1873 Hz in 0.5 s blocks: whatever else plays on these speakers barely moves the median.
    let level = std::sync::Arc::new(std::sync::Mutex::new(Vec::<f32>::new()));
    let capturing = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let capture = {
        let (level, capturing) = (level.clone(), capturing.clone());
        // SAFETY: plain COM calls on this thread; the mix format is 32-bit float, as asserted.
        std::thread::spawn(move || unsafe {
            use windows::Win32::Media::Audio::{AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, IAudioCaptureClient, IAudioClient};
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).unwrap();
            let client: IAudioClient = devices.GetDefaultAudioEndpoint(eRender, eConsole).unwrap().Activate(CLSCTX_ALL, None).unwrap();
            let format = client.GetMixFormat().unwrap();
            let (channels, rate) = (usize::from((*format).nChannels), f64::from((*format).nSamplesPerSec));
            let bits = (*format).wBitsPerSample;
            assert_eq!(bits, 32, "a float mix format");
            client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, 1_000_000, 0, format, None).unwrap();
            let reader: IAudioCaptureClient = client.GetService().unwrap();
            client.Start().unwrap();
            let block = (rate / 2.0) as usize;
            let mut samples: Vec<f32> = Vec::with_capacity(block);
            while capturing.load(std::sync::atomic::Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(10));
                while reader.GetNextPacketSize().unwrap_or(0) > 0 {
                    let (mut data, mut frames, mut flags) = (std::ptr::null_mut(), 0u32, 0u32);
                    if reader.GetBuffer(&mut data, &mut frames, &mut flags, None, None).is_err() {
                        break;
                    }
                    let mix = std::slice::from_raw_parts(data.cast::<f32>(), frames as usize * channels);
                    samples.extend(mix.iter().step_by(channels));
                    let _ = reader.ReleaseBuffer(frames);
                    while samples.len() >= block {
                        // Goertzel: the tone's amplitude in this block.
                        let coefficient = 2.0 * (std::f64::consts::TAU * 1_873.0 / rate).cos();
                        let (mut s1, mut s2) = (0f64, 0f64);
                        for sample in samples.drain(..block) {
                            let s0 = f64::from(sample) + coefficient * s1 - s2;
                            s2 = s1;
                            s1 = s0;
                        }
                        let amplitude = ((s1 * s1 + s2 * s2 - coefficient * s1 * s2).max(0.0).sqrt() * 2.0 / block as f64) as f32;
                        level.lock().unwrap().push(amplitude);
                    }
                }
            }
            let _ = client.Stop();
        })
    };
    // The median block since the last call.
    let take = || {
        let mut blocks = std::mem::take(&mut *level.lock().unwrap());
        blocks.sort_by(f32::total_cmp);
        blocks.get(blocks.len() / 2).copied().unwrap_or(0.0)
    };
    let level_over = |seconds: u64| {
        take();
        std::thread::sleep(Duration::from_secs(seconds));
        take()
    };
    std::thread::sleep(Duration::from_secs(3));
    let before = level_over(3);

    let during = tokio::runtime::Runtime::new().unwrap().block_on(async {
        let sender = PeerConnectionFactory::default();
        let viewer = PeerConnectionFactory::default();
        assert!(viewer.acquire_platform_adm());
        viewer.set_adm_playout_enabled(true);
        let alice = sender.create_peer_connection(RtcConfiguration::default()).unwrap();
        let bob = viewer.create_peer_connection(RtcConfiguration::default()).unwrap();
        let source = NativeAudioSource::new(AudioSourceOptions::default(), 48_000, 1, 0);
        alice.add_track(sender.create_audio_track("voice", source.clone()).into(), &["voice"]).unwrap();
        let (to_bob, from_alice) = std::sync::mpsc::channel();
        let (to_alice, from_bob) = std::sync::mpsc::channel();
        alice.on_ice_candidate(Some(Box::new(move |c| drop(to_bob.send((c.sdp_mid(), c.sdp_mline_index(), c.candidate()))))));
        bob.on_ice_candidate(Some(Box::new(move |c| drop(to_alice.send((c.sdp_mid(), c.sdp_mline_index(), c.candidate()))))));
        let offer = alice.create_offer(OfferOptions::default()).await.unwrap();
        alice.set_local_description(offer.clone()).await.unwrap();
        bob.set_remote_description(offer).await.unwrap();
        let answer = bob.create_answer(Default::default()).await.unwrap();
        bob.set_local_description(answer.clone()).await.unwrap();
        alice.set_remote_description(answer).await.unwrap();
        // Silence for 5 s: playing, not loudness, is what Windows ducks for.
        let end = Instant::now() + Duration::from_secs(6);
        let mut measuring = false;
        let silence = [0i16; 480];
        while Instant::now() < end {
            while let Ok((mid, index, candidate)) = from_alice.try_recv() {
                let _ = bob.add_ice_candidate(IceCandidate::parse(&mid, index, &candidate).unwrap()).await;
            }
            while let Ok((mid, index, candidate)) = from_bob.try_recv() {
                let _ = alice.add_ice_candidate(IceCandidate::parse(&mid, index, &candidate).unwrap()).await;
            }
            let frame = AudioFrame { data: silence.as_slice().into(), sample_rate: 48_000, num_channels: 1, samples_per_channel: 480 };
            let _ = source.capture_frame(&frame).await;
            if !measuring && end - Instant::now() < Duration::from_secs(3) {
                take();
                measuring = true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let level = take();
        alice.close();
        bob.close();
        level
    });
    capturing.store(false, std::sync::atomic::Ordering::Release);
    let _ = capture.join();
    let _ = other_app.kill();
    eprintln!("other app's tone: {before:.4} before, {during:.4} while the room played");
    assert!(before > 0.04, "the tone did not reach the speakers");
    assert!(during > before * 0.7, "other apps were lowered: {before:.4} -> {during:.4}");
}
