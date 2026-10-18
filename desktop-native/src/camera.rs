//! Webcam capture through Media Foundation, like the web's getUserMedia for the camera
//! (1280×720 ideal, at most 30 fps). Frames come out as RGB32 (B,G,R,X) and become I420 on a
//! NativeVideoSource, the same path as the screen.
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
    },
    thread::{self, JoinHandle},
    time::Instant,
};

use libwebrtc::{
    native::yuv_helper,
    video_frame::{I420Buffer, VideoFrame, VideoRotation},
    video_source::{VideoResolution, native::NativeVideoSource},
};
use windows::{
    Win32::{
        Media::MediaFoundation::*,
        System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize},
    },
    core::{GUID, Interface, PWSTR},
};

use crate::capture::CaptureEvent;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CameraDevice {
    /// The device's symbolic link, stable while it stays plugged in.
    pub id: String,
    pub label: String,
}

/// COM and Media Foundation for the current thread, released in order on drop.
struct Mf {
    com: bool,
}

impl Mf {
    fn start() -> Result<Self, String> {
        // SAFETY: plain per-thread initialization, balanced in Drop.
        let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
        if let Err(error) = unsafe { MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET) } {
            if com {
                unsafe { CoUninitialize() };
            }
            return Err(format!("CAPTURE_ERROR: Media Foundation indisponível ({error})."));
        }
        Ok(Self { com })
    }
}

impl Drop for Mf {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
            if self.com {
                CoUninitialize();
            }
        }
    }
}

fn activates() -> windows::core::Result<Vec<IMFActivate>> {
    unsafe {
        let mut attributes = None;
        MFCreateAttributes(&mut attributes, 1)?;
        let attributes = attributes.ok_or_else(windows::core::Error::empty)?;
        attributes.SetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID)?;
        let mut list = std::ptr::null_mut();
        let mut count = 0;
        MFEnumDeviceSources(&attributes, &mut list, &mut count)?;
        if list.is_null() {
            return Ok(Vec::new());
        }
        let found = std::slice::from_raw_parts_mut(list, count as usize).iter_mut().filter_map(Option::take).collect();
        CoTaskMemFree(Some(list as _));
        Ok(found)
    }
}

fn string(activate: &IMFActivate, key: &GUID) -> String {
    unsafe {
        let mut value = PWSTR::null();
        let mut length = 0;
        if activate.GetAllocatedString(key, &mut value, &mut length).is_err() {
            return String::new();
        }
        let text = value.to_string().unwrap_or_default();
        CoTaskMemFree(Some(value.0 as _));
        text
    }
}

/// The cameras plugged in now, for the dock's picker.
pub fn list() -> Result<Vec<CameraDevice>, String> {
    let _mf = Mf::start()?;
    let activates = activates().map_err(|error| format!("DEVICE_LOST: {error}"))?;
    Ok(activates
        .iter()
        .map(|activate| CameraDevice {
            id: string(activate, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK),
            label: string(activate, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME),
        })
        .collect())
}

/// How far a native format is from the web's ideal 1280×720 at up to 30 fps; lower is better.
/// Over 30 fps costs the most (the web caps at 30), then the distance in pixels.
pub fn format_cost(width: u32, height: u32, fps: f64) -> u64 {
    let over_rate = if fps > 30.5 { 1 << 40 } else { 0 };
    over_rate + (i64::from(width) * i64::from(height) - 1280 * 720).unsigned_abs()
}

fn unpack(value: u64) -> (u32, u32) {
    ((value >> 32) as u32, value as u32)
}

const STREAM: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;

fn open(device_id: Option<&str>) -> Result<(IMFSourceReader, u32, u32), String> {
    let error = |context: &str, error: windows::core::Error| {
        // E_ACCESSDENIED: Windows privacy settings block the camera for desktop apps.
        if error.code().0 as u32 == 0x8007_0005 {
            "PERMISSION_DENIED: o Windows bloqueou a câmera. Permita o acesso em Configurações > Privacidade > Câmera.".to_owned()
        } else {
            format!("CAPTURE_ERROR: {context} ({error})")
        }
    };
    let activates = activates().map_err(|e| error("listar câmeras", e))?;
    let activate = match device_id {
        Some(id) => activates.iter().find(|a| string(a, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK) == id),
        None => activates.first(),
    }
    .ok_or("DEVICE_LOST: câmera não encontrada. Escolha outra no dock.")?;
    unsafe {
        let source: IMFMediaSource = activate.ActivateObject().map_err(|e| error("abrir a câmera", e))?;
        let mut attributes = None;
        MFCreateAttributes(&mut attributes, 1).map_err(|e| error("configurar a câmera", e))?;
        let attributes = attributes.ok_or("CAPTURE_ERROR: configurar a câmera")?;
        // Lets the reader convert NV12/YUY2/MJPG to RGB32 for us.
        attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1).map_err(|e| error("configurar a câmera", e))?;
        let reader = MFCreateSourceReaderFromMediaSource(&source, &attributes).map_err(|e| error("abrir a câmera", e))?;
        let mut best: Option<(u64, IMFMediaType)> = None;
        for index in 0.. {
            let Ok(native) = reader.GetNativeMediaType(STREAM, index) else { break };
            let (width, height) = native.GetUINT64(&MF_MT_FRAME_SIZE).map(unpack).unwrap_or((0, 0));
            let (num, den) = native.GetUINT64(&MF_MT_FRAME_RATE).map(unpack).unwrap_or((30, 1));
            let cost = format_cost(width, height, f64::from(num) / f64::from(den.max(1)));
            if best.as_ref().is_none_or(|(known, _)| cost < *known) {
                best = Some((cost, native));
            }
        }
        if let Some((_, native)) = &best {
            reader.SetCurrentMediaType(STREAM, None, native).map_err(|e| error("escolher o formato", e))?;
        }
        let output = MFCreateMediaType().map_err(|e| error("escolher o formato", e))?;
        output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(|e| error("escolher o formato", e))?;
        output.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).map_err(|e| error("escolher o formato", e))?;
        reader.SetCurrentMediaType(STREAM, None, &output).map_err(|e| error("converter o formato", e))?;
        let current = reader.GetCurrentMediaType(STREAM).map_err(|e| error("ler o formato", e))?;
        let (width, height) = current.GetUINT64(&MF_MT_FRAME_SIZE).map(unpack).map_err(|e| error("ler o formato", e))?;
        Ok((reader, width, height))
    }
}

/// Copies one RGB32 sample into a top-down B,G,R,A image (`pitch` < 0 means bottom-up).
fn frame_bytes(sample: &IMFSample, width: u32, height: u32) -> Option<Vec<u8>> {
    let row = width as usize * 4;
    let mut image = vec![0; row * height as usize];
    unsafe {
        let buffer = sample.GetBufferByIndex(0).ok()?;
        if let Ok(buffer) = buffer.cast::<IMF2DBuffer>() {
            let (mut scan0, mut pitch) = (std::ptr::null_mut(), 0);
            buffer.Lock2D(&mut scan0, &mut pitch).ok()?;
            for (y, out) in image.chunks_exact_mut(row).enumerate() {
                out.copy_from_slice(std::slice::from_raw_parts(scan0.offset(y as isize * pitch as isize), row));
            }
            let _ = buffer.Unlock2D();
        } else {
            let (mut data, mut length) = (std::ptr::null_mut(), 0);
            buffer.Lock(&mut data, None, Some(&mut length)).ok()?;
            let size = image.len();
            let copied = (length as usize >= size).then(|| image.copy_from_slice(std::slice::from_raw_parts(data, size)));
            let _ = buffer.Unlock();
            copied?;
        }
    }
    Some(image)
}

pub struct Camera {
    pub source: NativeVideoSource,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Camera {
    /// Opens the camera on its own thread; `CameraReady` follows the first frame and
    /// `CameraEnded` any failure, like the screen capture.
    pub fn start(device_id: Option<String>, generation: u64, runtime: &tokio::runtime::Runtime, events: SyncSender<CaptureEvent>) -> Self {
        let _runtime = runtime.enter();
        let source = NativeVideoSource::new(VideoResolution { width: 1280, height: 720 }, false);
        let running = Arc::new(AtomicBool::new(true));
        let (thread_source, thread_running) = (source.clone(), running.clone());
        let thread = thread::spawn(move || {
            let result = (|| -> Result<(), String> {
                let _mf = Mf::start()?;
                let (reader, width, height) = open(device_id.as_deref())?;
                let started = Instant::now();
                let mut ready = false;
                while thread_running.load(Ordering::Acquire) {
                    let (mut flags, mut sample) = (0, None);
                    unsafe { reader.ReadSample(STREAM, 0, None, Some(&mut flags), None, Some(&mut sample)) }
                        .map_err(|error| format!("DEVICE_LOST: a câmera parou ({error})."))?;
                    if flags & (MF_SOURCE_READERF_ERROR.0 | MF_SOURCE_READERF_ENDOFSTREAM.0) as u32 != 0 {
                        return Err("DEVICE_LOST: a câmera foi desconectada.".into());
                    }
                    let Some(image) = sample.and_then(|sample| frame_bytes(&sample, width, height)) else { continue };
                    let mut buffer = I420Buffer::new(width, height);
                    let (sy, su, sv) = buffer.strides();
                    let (y, u, v) = buffer.data_mut();
                    // libyuv's "ARGB" is B,G,R,A in memory, as RGB32.
                    yuv_helper::argb_to_i420(&image, width * 4, y, sy, u, su, v, sv, width as i32, height as i32);
                    thread_source.capture_frame(&VideoFrame {
                        rotation: VideoRotation::VideoRotation0,
                        timestamp_us: started.elapsed().as_micros() as i64,
                        frame_metadata: None,
                        buffer,
                    });
                    if !ready {
                        ready = true;
                        let _ = events.try_send(CaptureEvent::CameraReady(generation));
                    }
                }
                Ok(())
            })();
            if let Err(reason) = result {
                if thread_running.load(Ordering::Acquire) {
                    let _ = events.try_send(CaptureEvent::CameraEnded(generation, reason));
                }
            }
        });
        Self { source, running, thread: Some(thread) }
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        // ReadSample returns with the next frame (≤ 33 ms), then the loop sees the flag.
        self.running.store(false, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_720p_and_never_over_30_fps() {
        let pick = |formats: &[(u32, u32, f64)]| {
            formats.iter().min_by_key(|(w, h, fps)| format_cost(*w, *h, *fps)).copied().unwrap()
        };
        assert_eq!(pick(&[(640, 480, 30.0), (1280, 720, 30.0), (1920, 1080, 30.0)]), (1280, 720, 30.0));
        assert_eq!(pick(&[(1280, 720, 60.0), (1920, 1080, 30.0)]), (1920, 1080, 30.0), "60 fps loses to any 30 fps format");
        assert_eq!(pick(&[(640, 480, 30.0), (1920, 1080, 30.0)]), (640, 480, 30.0), "the closest pixel count wins");
    }
}
