//! Native video capture without the WebView picker, sharing bar or capture border:
//! DXGI Desktop Duplication for screens, Windows.Graphics.Capture for windows, and
//! PrintWindow for windows where WGC is missing (older Windows 10 builds).
//! Frames are BGRA, top-down, tightly packed (stride = width * 4) and paced to the
//! requested frame rate; a busy consumer drops frames instead of queueing them.
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows::core::{Interface, BOOL};
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureAccess,
    GraphicsCaptureAccessKind, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::{HMODULE, HWND, RECT};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, ID3D11VideoContext,
    ID3D11VideoDevice, ID3D11VideoProcessor, ID3D11VideoProcessorInputView,
    ID3D11VideoProcessorOutputView, D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE,
    D3D11_BOX, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE,
    D3D11_MAP_READ, D3D11_SDK_VERSION, D3D11_TEX2D_VPIV, D3D11_TEX2D_VPOV, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING, D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
    D3D11_VIDEO_PROCESSOR_CONTENT_DESC, D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC,
    D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0, D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC,
    D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0, D3D11_VIDEO_PROCESSOR_STREAM,
    D3D11_VIDEO_USAGE_OPTIMAL_SPEED, D3D11_VPIV_DIMENSION_TEXTURE2D,
    D3D11_VPOV_DIMENSION_TEXTURE2D,
};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_RATIONAL, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIDevice, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication,
    IDXGIResource, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
    DXGI_OUTDUPL_POINTER_SHAPE_INFO, DXGI_OUTDUPL_POINTER_SHAPE_TYPE_COLOR,
    DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MASKED_COLOR, DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME,
};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetWindowDC, ReleaseDC,
    SelectObject, SetBrushOrgEx, SetStretchBltMode, StretchBlt, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HALFTONE, HBITMAP, HDC, HGDIOBJ, SRCCOPY,
};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, IsIconic, IsWindow};

#[link(name = "user32")]
unsafe extern "system" {
    fn PrintWindow(hwnd: HWND, hdc_blt: HDC, flags: u32) -> BOOL;
}
const PW_RENDERFULLCONTENT: u32 = 2;
const WINDOW_MAX_FPS: u32 = 30;

#[derive(Clone, Copy, Debug)]
pub enum VideoSource {
    /// Desktop rectangle (virtual-screen coordinates) inside one monitor.
    Screen {
        left: i32,
        top: i32,
        width: u32,
        height: u32,
    },
    Window(HWND),
}
unsafe impl Send for VideoSource {}

pub struct Frame<'a> {
    pub width: u32,
    pub height: u32,
    pub data: &'a [u8],
    /// Monotonic since capture start; used as the VideoFrame timestamp.
    pub timestamp_us: u64,
    /// Wall clock at capture, comparable with the WebView's Date.now() for latency.
    pub wall_us: u64,
}

#[derive(Default)]
pub struct VideoMetrics {
    pub ticks: AtomicU64,
    pub new_frames: AtomicU64,
    pub delivered: AtomicU64,
    pub dropped_busy: AtomicU64,
    pub late_ticks: AtomicU64,
    pub capture_us_total: AtomicU64,
    pub capture_us_max: AtomicU64,
    pub minimized_ticks: AtomicU64,
}

/// Runs until `running` is cleared or the source fails. `deliver` returns false when
/// the consumer had no free slot; that frame is counted as dropped and never queued.
/// `fps` and `max_height` are read every tick, so both can change while the capture runs;
/// a taller source is scaled down to `max_height` (on the GPU for screens) before readback.
pub fn capture_video_loop(
    source: VideoSource,
    fps: Arc<AtomicU32>,
    max_height: Arc<AtomicU32>,
    running: Arc<AtomicBool>,
    metrics: Arc<VideoMetrics>,
    mut deliver: impl FnMut(Frame) -> bool,
) -> Result<(), String> {
    let mut grabber: Box<dyn Grabber> = match source {
        VideoSource::Screen {
            left,
            top,
            width,
            height,
        } => Box::new(DxgiGrabber::new(left, top, even(width), even(height))?),
        // PrintWindow covers Windows 10 builds without WGC and windows WGC refuses.
        VideoSource::Window(hwnd) => match WgcGrabber::new(hwnd) {
            Ok(grabber) => Box::new(grabber),
            Err(_) => Box::new(WindowGrabber::new(hwnd)),
        },
    };
    let start = Instant::now();
    let mut next = start;
    while running.load(Ordering::Acquire) {
        let interval = frame_interval(fps.load(Ordering::Relaxed), grabber.max_fps());
        let now = Instant::now();
        if now < next {
            std::thread::sleep(next - now);
        } else if now - next > interval {
            // Too slow for the requested rate: skip ticks instead of bursting.
            metrics.late_ticks.fetch_add(1, Ordering::Relaxed);
            next = now;
        }
        next += interval;
        metrics.ticks.fetch_add(1, Ordering::Relaxed);
        let began = Instant::now();
        let grabbed = grabber.grab(&metrics, max_height.load(Ordering::Relaxed))?;
        let spent = began.elapsed().as_micros() as u64;
        metrics.capture_us_total.fetch_add(spent, Ordering::Relaxed);
        metrics.capture_us_max.fetch_max(spent, Ordering::Relaxed);
        let Some((width, height, data)) = grabbed else {
            continue;
        };
        let frame = Frame {
            width,
            height,
            data,
            timestamp_us: start.elapsed().as_micros() as u64,
            wall_us: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_micros() as u64)
                .unwrap_or(0),
        };
        if deliver(frame) {
            metrics.delivered.fetch_add(1, Ordering::Relaxed);
        } else {
            metrics.dropped_busy.fetch_add(1, Ordering::Relaxed);
        }
    }
    Ok(())
}

fn frame_interval(fps: u32, max_fps: u32) -> Duration {
    Duration::from_micros(1_000_000 / u64::from(fps.min(max_fps).clamp(1, 120)))
}

fn even(value: u32) -> u32 {
    (value & !1).max(2)
}

/// Even size with the source's aspect at `max_height`, or the source size when it fits.
fn scaled(width: u32, height: u32, max_height: u32) -> (u32, u32) {
    if height <= max_height {
        return (width, height);
    }
    let w = u64::from(width) * u64::from(max_height) / u64::from(height);
    (even(w as u32), even(max_height))
}

/// Width, height and BGRA pixels of the latest image.
type Grab<'a> = Result<Option<(u32, u32, &'a [u8])>, String>;

trait Grabber {
    /// Returns the latest image no taller than `max_height` (repeating the previous one
    /// when nothing changed), or None when there is nothing to send yet.
    fn grab(&mut self, metrics: &VideoMetrics, max_height: u32) -> Grab<'_>;

    /// Highest rate this grabber keeps up with.
    fn max_fps(&self) -> u32 {
        u32::MAX
    }
}

/// GPU half shared by screens and WGC windows: crops the captured texture, scales it to
/// `max_height` and reads it back without waiting on the GPU.
struct Readback {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    // Two readback textures: the CPU maps the one copied a tick ago, so it never waits
    // for the GPU to finish this tick's copy (which would stall on the game).
    staging: [Option<ID3D11Texture2D>; 2],
    // Slot the next copy goes to; the other one holds the copy not read yet, if any.
    next: usize,
    unread: bool,
    // Crop inside the captured texture.
    crop: (u32, u32, u32, u32),
    // Size of the delivered image: the crop, or the crop scaled down by `scaler`.
    size: (u32, u32),
    scaler: Option<Scaler>,
    // The limit `size` was computed for; None until the first configure.
    max_height: Option<u32>,
    // Latest image read back, `size` big.
    image: Vec<u8>,
}

impl Readback {
    fn new(device: ID3D11Device, context: ID3D11DeviceContext) -> Self {
        Self {
            device,
            context,
            staging: [None, None],
            next: 0,
            unread: false,
            crop: (0, 0, 0, 0),
            size: (0, 0),
            scaler: None,
            max_height: None,
            image: Vec::new(),
        }
    }

    /// Points the pipeline at `crop` scaled to `max_height`. Returns true when that changed
    /// anything, which drops the pending copy and clears `image`. Without a GPU video
    /// processor the crop goes out unscaled, as before scaling existed.
    fn configure(&mut self, crop: (u32, u32, u32, u32), max_height: u32) -> bool {
        if self.crop == crop && self.max_height == Some(max_height) {
            return false;
        }
        self.crop = crop;
        self.max_height = Some(max_height);
        let (_, _, w, h) = crop;
        let mut size = scaled(w, h, max_height);
        self.scaler = None;
        if size != (w, h) {
            match unsafe { Scaler::new(&self.device, &self.context, (w, h), size) } {
                Ok(scaler) => self.scaler = Some(scaler),
                Err(_) => size = (w, h),
            }
        }
        self.size = size;
        self.staging = [None, None];
        self.unread = false;
        self.image = vec![0; (size.0 * size.1 * 4) as usize];
        true
    }

    /// Queues this tick's copy of the crop of `texture`, scaled, into a staging texture.
    unsafe fn copy(&mut self, texture: &ID3D11Texture2D) -> Result<(), String> {
        let (x, y, w, h) = self.crop;
        let slot = &mut self.staging[self.next];
        if slot.is_none() {
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            texture.GetDesc(&mut desc);
            (desc.Width, desc.Height) = self.size;
            desc.MipLevels = 1;
            desc.ArraySize = 1;
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            desc.MiscFlags = 0;
            self.device
                .CreateTexture2D(&desc, None, Some(slot))
                .map_err(|_| "dxgi-capture-failed")?;
        }
        let crop = D3D11_BOX {
            left: x,
            top: y,
            front: 0,
            right: x + w,
            bottom: y + h,
            back: 1,
        };
        let slot = slot.as_ref().unwrap();
        match &self.scaler {
            Some(scaler) => scaler.run(&self.context, texture, &crop, slot)?,
            None => self
                .context
                .CopySubresourceRegion(slot, 0, 0, 0, 0, texture, 0, Some(&crop)),
        }
        Ok(())
    }

    /// Reads last tick's copy, which the GPU has finished, into `image`; this tick's copy
    /// (`copied`) waits for the next tick, so a still source shows its last change one
    /// interval later. Returns whether `image` changed.
    fn finish(&mut self, copied: bool) -> Result<bool, String> {
        let image_changed = std::mem::replace(&mut self.unread, copied);
        if image_changed {
            self.read_staging(self.next ^ 1)?;
        }
        if copied {
            self.next ^= 1;
        }
        Ok(image_changed)
    }

    fn read_staging(&mut self, slot: usize) -> Result<(), String> {
        let Some(staging) = &self.staging[slot] else {
            return Ok(());
        };
        unsafe {
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(|_| "dxgi-capture-failed")?;
            let (w, h) = self.size;
            let row = (w * 4) as usize;
            for line in 0..h as usize {
                let src = (mapped.pData as *const u8).add(line * mapped.RowPitch as usize);
                std::ptr::copy_nonoverlapping(src, self.image[line * row..].as_mut_ptr(), row);
            }
            self.context.Unmap(staging, 0);
        }
        Ok(())
    }
}

struct DxgiGrabber {
    output: IDXGIOutput1,
    duplication: Option<IDXGIOutputDuplication>,
    // Crop inside the output, in output pixels.
    crop: (u32, u32, u32, u32),
    // Its `image` is the desktop without the pointer, which DXGI reports separately.
    readback: Readback,
    buffer: Vec<u8>,
    has_frame: bool,
    pointer: Option<PointerShape>,
    // Top-left of the pointer image in output pixels; None while it is elsewhere or hidden.
    pointer_at: Option<(i32, i32)>,
}

impl DxgiGrabber {
    fn new(left: i32, top: i32, width: u32, height: u32) -> Result<Self, String> {
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1().map_err(|_| "dxgi-unavailable")?;
            let mut index = 0;
            while let Ok(adapter) = factory.EnumAdapters1(index) {
                index += 1;
                let mut output_index = 0;
                while let Ok(output) = adapter.EnumOutputs(output_index) {
                    output_index += 1;
                    let desc = output.GetDesc().map_err(|_| "dxgi-output-unavailable")?;
                    let r = desc.DesktopCoordinates;
                    if left < r.left || top < r.top || left >= r.right || top >= r.bottom {
                        continue;
                    }
                    let crop_x = (left - r.left) as u32;
                    let crop_y = (top - r.top) as u32;
                    let width = width.min((r.right - r.left) as u32 - crop_x) & !1;
                    let height = height.min((r.bottom - r.top) as u32 - crop_y) & !1;
                    let (device, context) = create_device(&adapter)?;
                    let output: IDXGIOutput1 =
                        output.cast().map_err(|_| "dxgi-duplication-unsupported")?;
                    return Ok(Self {
                        output,
                        duplication: None,
                        crop: (crop_x, crop_y, width, height),
                        readback: Readback::new(device, context),
                        buffer: Vec::new(),
                        has_frame: false,
                        pointer: None,
                        pointer_at: None,
                    });
                }
            }
            Err("screen-not-found".into())
        }
    }

    /// Returns (desktop image copied, pointer changed).
    fn acquire(&mut self) -> Result<(bool, bool), String> {
        unsafe {
            if self.duplication.is_none() {
                // Fails transiently on the secure desktop (UAC, lock screen): retry next tick.
                self.duplication = self.output.DuplicateOutput(&self.readback.device).ok();
            }
            let Some(duplication) = &self.duplication else {
                return Ok((false, false));
            };
            let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource: Option<IDXGIResource> = None;
            match duplication.AcquireNextFrame(0, &mut info, &mut resource) {
                Ok(()) => {}
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok((false, false)),
                Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST => {
                    self.duplication = None;
                    return Ok((false, false));
                }
                Err(_) => return Err("dxgi-capture-failed".into()),
            }
            let pointer_changed = info.LastMouseUpdateTime != 0;
            if pointer_changed {
                let position = info.PointerPosition;
                self.pointer_at = position
                    .Visible
                    .as_bool()
                    .then_some((position.Position.x, position.Position.y));
            }
            if info.PointerShapeBufferSize > 0 {
                let mut data = vec![0u8; info.PointerShapeBufferSize as usize];
                let mut required = 0;
                let mut shape = DXGI_OUTDUPL_POINTER_SHAPE_INFO::default();
                if duplication
                    .GetFramePointerShape(
                        data.len() as u32,
                        data.as_mut_ptr().cast(),
                        &mut required,
                        &mut shape,
                    )
                    .is_ok()
                {
                    let kind = match shape.Type as i32 {
                        t if t == DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME.0 => {
                            Some(PointerKind::Monochrome)
                        }
                        t if t == DXGI_OUTDUPL_POINTER_SHAPE_TYPE_COLOR.0 => {
                            Some(PointerKind::Color)
                        }
                        t if t == DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MASKED_COLOR.0 => {
                            Some(PointerKind::MaskedColor)
                        }
                        _ => None,
                    };
                    self.pointer = kind.map(|kind| PointerShape {
                        kind,
                        width: shape.Width,
                        height: if kind == PointerKind::Monochrome {
                            shape.Height / 2
                        } else {
                            shape.Height
                        },
                        pitch: shape.Pitch,
                        data,
                    });
                }
            }
            let result = (|| {
                // Pointer-only updates carry no new desktop image.
                if info.LastPresentTime == 0 {
                    return Ok(false);
                }
                let texture: ID3D11Texture2D = resource
                    .ok_or("dxgi-capture-failed")?
                    .cast()
                    .map_err(|_| "dxgi-capture-failed")?;
                self.readback.copy(&texture)?;
                Ok::<bool, String>(true)
            })();
            let _ = duplication.ReleaseFrame();
            Ok((result?, pointer_changed))
        }
    }
}

impl Grabber for DxgiGrabber {
    fn grab(&mut self, metrics: &VideoMetrics, max_height: u32) -> Grab<'_> {
        if self.readback.configure(self.crop, max_height) {
            let (w, h) = self.readback.size;
            self.has_frame = false;
            self.buffer = vec![0; (w * h * 4) as usize];
        }
        let (copied, pointer_changed) = self.acquire()?;
        let image_changed = self.readback.finish(copied)?;
        if copied {
            metrics.new_frames.fetch_add(1, Ordering::Relaxed);
        }
        if !self.has_frame && !image_changed {
            return Ok(None);
        }
        let (x, y, crop_w, crop_h) = self.crop;
        let (w, h) = self.readback.size;
        if image_changed || pointer_changed {
            self.has_frame = true;
            self.buffer.copy_from_slice(&self.readback.image);
            if let (Some(shape), Some((px, py))) = (&self.pointer, self.pointer_at) {
                // shortcut: the pointer shape is not scaled, so it looks bigger on a scaled
                // image; scale it too if anyone notices.
                let px = (px - x as i32) * w as i32 / crop_w as i32;
                let py = (py - y as i32) * h as i32 / crop_h as i32;
                draw_pointer(&mut self.buffer, w, h, shape, px, py);
            }
        }
        Ok(Some((w, h, &self.buffer)))
    }
}

/// Scales the crop on the GPU with the D3D11 video processor, so readback, the copies after
/// it and the encoder's color conversion all handle the smaller image.
struct Scaler {
    video: ID3D11VideoContext,
    processor: ID3D11VideoProcessor,
    // The crop lands here first: captured textures can't back a processor input view.
    input: ID3D11Texture2D,
    input_view: ID3D11VideoProcessorInputView,
    output: ID3D11Texture2D,
    output_view: ID3D11VideoProcessorOutputView,
}

impl Scaler {
    unsafe fn new(
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        (width, height): (u32, u32),
        (out_width, out_height): (u32, u32),
    ) -> Result<Self, String> {
        const FAILED: &str = "video-processor-unavailable";
        let video_device: ID3D11VideoDevice = device.cast().map_err(|_| FAILED)?;
        let video: ID3D11VideoContext = context.cast().map_err(|_| FAILED)?;
        let rate = DXGI_RATIONAL {
            Numerator: 60,
            Denominator: 1,
        };
        let content = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
            InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
            InputFrameRate: rate,
            InputWidth: width,
            InputHeight: height,
            OutputFrameRate: rate,
            OutputWidth: out_width,
            OutputHeight: out_height,
            Usage: D3D11_VIDEO_USAGE_OPTIMAL_SPEED,
        };
        let enumerator = video_device
            .CreateVideoProcessorEnumerator(&content)
            .map_err(|_| FAILED)?;
        let processor = video_device
            .CreateVideoProcessor(&enumerator, 0)
            .map_err(|_| FAILED)?;
        // No driver "enhancements" (denoise, edge enhancement) on a desktop image.
        video.VideoProcessorSetStreamAutoProcessingMode(&processor, 0, false);
        let texture = |width, height| {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                // DuplicateOutput and the WGC frame pool both hand out BGRA8.
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
                ..Default::default()
            };
            let mut texture = None;
            device
                .CreateTexture2D(&desc, None, Some(&mut texture))
                .map_err(|_| FAILED)?;
            texture.ok_or(FAILED)
        };
        let input = texture(width, height)?;
        let output = texture(out_width, out_height)?;
        let mut input_view = None;
        video_device
            .CreateVideoProcessorInputView(
                &input,
                &enumerator,
                &D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
                    FourCC: 0,
                    ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
                    Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 {
                        Texture2D: D3D11_TEX2D_VPIV {
                            MipSlice: 0,
                            ArraySlice: 0,
                        },
                    },
                },
                Some(&mut input_view),
            )
            .map_err(|_| FAILED)?;
        let mut output_view = None;
        video_device
            .CreateVideoProcessorOutputView(
                &output,
                &enumerator,
                &D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
                    ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
                    Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 {
                        Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 },
                    },
                },
                Some(&mut output_view),
            )
            .map_err(|_| FAILED)?;
        Ok(Self {
            video,
            processor,
            input,
            input_view: input_view.ok_or(FAILED)?,
            output,
            output_view: output_view.ok_or(FAILED)?,
        })
    }

    /// Copies `crop` of `source` scaled into `target`, all queued on the GPU.
    unsafe fn run(
        &self,
        context: &ID3D11DeviceContext,
        source: &ID3D11Texture2D,
        crop: &D3D11_BOX,
        target: &ID3D11Texture2D,
    ) -> Result<(), String> {
        context.CopySubresourceRegion(&self.input, 0, 0, 0, 0, source, 0, Some(crop));
        let mut streams = [D3D11_VIDEO_PROCESSOR_STREAM {
            Enable: true.into(),
            pInputSurface: ManuallyDrop::new(Some(self.input_view.clone())),
            ..Default::default()
        }];
        let result = self
            .video
            .VideoProcessorBlt(&self.processor, &self.output_view, 0, &streams);
        ManuallyDrop::drop(&mut streams[0].pInputSurface);
        result.map_err(|_| "dxgi-capture-failed")?;
        context.CopyResource(target, &self.output);
        Ok(())
    }
}

fn create_device(adapter: &IDXGIAdapter1) -> Result<(ID3D11Device, ID3D11DeviceContext), String> {
    let mut device = None;
    let mut context = None;
    unsafe {
        D3D11CreateDevice(
            adapter,
            D3D_DRIVER_TYPE_UNKNOWN,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .map_err(|_| "d3d11-unavailable")?;
    }
    Ok((
        device.ok_or("d3d11-unavailable")?,
        context.ok_or("d3d11-unavailable")?,
    ))
}

const WGC_FORMAT: DirectXPixelFormat = DirectXPixelFormat::B8G8R8A8UIntNormalized;
const WGC_BUFFERS: i32 = 2;

/// Windows.Graphics.Capture: DWM hands over the window's surface as a texture, so a game in
/// a window costs a GPU copy instead of PrintWindow's ~25 ms redraw.
struct WgcGrabber {
    hwnd: HWND,
    device: IDirect3DDevice,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    // Size the pool's buffers were created for.
    pool_size: SizeInt32,
    readback: Readback,
    has_frame: bool,
}

impl WgcGrabber {
    fn new(hwnd: HWND) -> Result<Self, String> {
        const FAILED: &str = "wgc-unavailable";
        unsafe {
            // WinRT on this capture thread; S_FALSE or a different mode set earlier are fine.
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
            if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
                return Err(FAILED.into());
            }
            // shortcut: the first adapter, not the one drawing the window; a hybrid-GPU laptop
            // pays a cross-adapter copy, pick the window's adapter if that shows up.
            let factory: IDXGIFactory1 = CreateDXGIFactory1().map_err(|_| FAILED)?;
            let adapter = factory.EnumAdapters1(0).map_err(|_| FAILED)?;
            let (d3d, context) = create_device(&adapter)?;
            let dxgi: IDXGIDevice = d3d.cast().map_err(|_| FAILED)?;
            let device: IDirect3DDevice = CreateDirect3D11DeviceFromDXGIDevice(&dxgi)
                .and_then(|d| d.cast())
                .map_err(|_| FAILED)?;
            let item: GraphicsCaptureItem =
                windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
                    .and_then(|interop| interop.CreateForWindow(hwnd))
                    .map_err(|_| FAILED)?;
            let pool_size = item.Size().map_err(|_| FAILED)?;
            let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
                &device,
                WGC_FORMAT,
                WGC_BUFFERS,
                pool_size,
            )
            .map_err(|_| FAILED)?;
            let session = pool.CreateCaptureSession(&item).map_err(|_| FAILED)?;
            // No pointer, as with PrintWindow, and no yellow border around the shared window;
            // both setters fail on builds that predate them (the border one needs Windows 11).
            let _ = session.SetIsCursorCaptureEnabled(false);
            if let Ok(access) = GraphicsCaptureAccess::RequestAccessAsync(
                GraphicsCaptureAccessKind::Borderless,
            ) {
                let _ = access.join();
            }
            let _ = session.SetIsBorderRequired(false);
            session.StartCapture().map_err(|_| FAILED)?;
            Ok(Self {
                hwnd,
                device,
                pool,
                session,
                pool_size,
                readback: Readback::new(d3d, context),
                has_frame: false,
            })
        }
    }

    /// Queues the readback of `frame`; returns false when it had to be skipped.
    fn copy(&mut self, frame: &Direct3D11CaptureFrame, max_height: u32) -> Result<bool, String> {
        const FAILED: &str = "window-capture-failed";
        let size = frame.ContentSize().map_err(|_| FAILED)?;
        if size != self.pool_size {
            // The window was resized: frames come at the new size once the pool matches it.
            self.pool
                .Recreate(&self.device, WGC_FORMAT, WGC_BUFFERS, size)
                .map_err(|_| FAILED)?;
            self.pool_size = size;
            return Ok(false);
        }
        unsafe {
            let texture: ID3D11Texture2D = frame
                .Surface()
                .and_then(|s| s.cast::<IDirect3DDxgiInterfaceAccess>())
                .and_then(|access| access.GetInterface())
                .map_err(|_| FAILED)?;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            texture.GetDesc(&mut desc);
            let w = (size.Width.max(0) as u32).min(desc.Width);
            let h = (size.Height.max(0) as u32).min(desc.Height);
            if w < 2 || h < 2 {
                return Ok(false);
            }
            if self.readback.configure((0, 0, even(w), even(h)), max_height) {
                self.has_frame = false;
            }
            self.readback.copy(&texture)?;
        }
        Ok(true)
    }
}

impl Drop for WgcGrabber {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}

impl Grabber for WgcGrabber {
    fn grab(&mut self, metrics: &VideoMetrics, max_height: u32) -> Grab<'_> {
        unsafe {
            if !IsWindow(Some(self.hwnd)).as_bool() {
                return Err("window-closed".into());
            }
            if IsIconic(self.hwnd).as_bool() {
                // No frames arrive while minimized: the last image keeps going out.
                metrics.minimized_ticks.fetch_add(1, Ordering::Relaxed);
            }
        }
        // Only the newest frame matters; the older ones go back to the pool unread.
        let mut latest = None;
        while let Ok(frame) = self.pool.TryGetNextFrame() {
            latest = Some(frame);
        }
        let copied = match latest {
            Some(frame) => {
                let copied = self.copy(&frame, max_height);
                let _ = frame.Close();
                copied?
            }
            None => false,
        };
        let image_changed = self.readback.finish(copied)?;
        if copied {
            metrics.new_frames.fetch_add(1, Ordering::Relaxed);
        }
        self.has_frame |= image_changed;
        if !self.has_frame {
            return Ok(None);
        }
        let (w, h) = self.readback.size;
        Ok(Some((w, h, &self.readback.image)))
    }
}

struct WindowGrabber {
    hwnd: HWND,
    dib: Option<Dib>,
    // Target of the GDI downscale when the window is taller than the limit.
    scaled: Option<Dib>,
    // Size of the delivered image.
    size: (u32, u32),
    buffer: Vec<u8>,
    has_frame: bool,
}

struct Dib {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    bits: *const u8,
    width: u32,
    height: u32,
}

impl Drop for Dib {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            let _ = DeleteDC(self.dc);
        }
    }
}

impl WindowGrabber {
    fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            dib: None,
            scaled: None,
            size: (0, 0),
            buffer: Vec::new(),
            has_frame: false,
        }
    }
}

/// Makes `slot` hold a top-down 32-bit DIB of this size, compatible with the window's DC.
fn ensure_dib(slot: &mut Option<Dib>, hwnd: HWND, width: u32, height: u32) -> Result<(), String> {
    if slot
        .as_ref()
        .is_some_and(|d| d.width == width && d.height == height)
    {
        return Ok(());
    }
    *slot = None;
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        bmiColors: [Default::default()],
    };
    unsafe {
        let window_dc = GetWindowDC(Some(hwnd));
        if window_dc.0.is_null() {
            return Err("window-capture-failed".into());
        }
        let dc = CreateCompatibleDC(Some(window_dc));
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let bitmap = CreateDIBSection(Some(window_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0);
        let _ = ReleaseDC(Some(hwnd), window_dc);
        let bitmap = match bitmap {
            Ok(bitmap) if !bits.is_null() && !dc.0.is_null() => bitmap,
            _ => {
                let _ = DeleteDC(dc);
                return Err("window-capture-failed".into());
            }
        };
        let old = SelectObject(dc, HGDIOBJ(bitmap.0));
        *slot = Some(Dib {
            dc,
            bitmap,
            old,
            bits: bits as *const u8,
            width,
            height,
        });
    }
    Ok(())
}

impl Grabber for WindowGrabber {
    fn max_fps(&self) -> u32 {
        // PrintWindow costs ~25 ms per capture and saturates near 35 fps on Windows 10.
        WINDOW_MAX_FPS
    }

    fn grab(&mut self, metrics: &VideoMetrics, max_height: u32) -> Grab<'_> {
        unsafe {
            if !IsWindow(Some(self.hwnd)).as_bool() {
                return Err("window-closed".into());
            }
            if IsIconic(self.hwnd).as_bool() {
                // A minimized window has no surface: keep sending the last image.
                metrics.minimized_ticks.fetch_add(1, Ordering::Relaxed);
            } else {
                let mut rect = RECT::default();
                let _ = GetWindowRect(self.hwnd, &mut rect);
                let mut frame = rect;
                let _ = DwmGetWindowAttribute(
                    self.hwnd,
                    DWMWA_EXTENDED_FRAME_BOUNDS,
                    &mut frame as *mut RECT as *mut _,
                    std::mem::size_of::<RECT>() as u32,
                );
                let width = (rect.right - rect.left).max(0) as u32;
                let height = (rect.bottom - rect.top).max(0) as u32;
                if width < 2 || height < 2 {
                    return Ok(None);
                }
                ensure_dib(&mut self.dib, self.hwnd, width, height)?;
                let dib = self.dib.as_ref().unwrap();
                if PrintWindow(self.hwnd, dib.dc, PW_RENDERFULLCONTENT).as_bool() {
                    // Visible frame inside the window rect (Windows 10 adds invisible resize borders).
                    let x = (frame.left - rect.left).clamp(0, width as i32) as u32;
                    let y = (frame.top - rect.top).clamp(0, height as i32) as u32;
                    let w = even(((frame.right - frame.left).max(0) as u32).min(width - x));
                    let h = even(((frame.bottom - frame.top).max(0) as u32).min(height - y));
                    self.size = scaled(w, h, max_height);
                    let (out_w, out_h) = self.size;
                    // Rows of the image to copy: the visible part of the window, or all of the
                    // scaled DIB, which HALFTONE StretchBlt filled from it.
                    let (source, stride, x, y) = if (out_w, out_h) == (w, h) {
                        (dib.bits, width, x, y)
                    } else {
                        ensure_dib(&mut self.scaled, self.hwnd, out_w, out_h)?;
                        let scaled = self.scaled.as_ref().unwrap();
                        SetStretchBltMode(scaled.dc, HALFTONE);
                        let _ = SetBrushOrgEx(scaled.dc, 0, 0, None);
                        let (out_w, out_h, w, h) = (out_w as i32, out_h as i32, w as i32, h as i32);
                        let _ = StretchBlt(
                            scaled.dc,
                            0,
                            0,
                            out_w,
                            out_h,
                            Some(dib.dc),
                            x as i32,
                            y as i32,
                            w,
                            h,
                            SRCCOPY,
                        );
                        (scaled.bits, self.size.0, 0, 0)
                    };
                    let row = (out_w * 4) as usize;
                    self.buffer.resize(row * out_h as usize, 0);
                    for line in 0..out_h as usize {
                        let src =
                            source.add(((y as usize + line) * stride as usize + x as usize) * 4);
                        std::ptr::copy_nonoverlapping(
                            src,
                            self.buffer[line * row..].as_mut_ptr(),
                            row,
                        );
                    }
                    metrics.new_frames.fetch_add(1, Ordering::Relaxed);
                    self.has_frame = true;
                }
            }
        }
        if !self.has_frame {
            return Ok(None);
        }
        let (w, h) = self.size;
        Ok(Some((w, h, &self.buffer)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PointerKind {
    Monochrome,
    Color,
    MaskedColor,
}

struct PointerShape {
    kind: PointerKind,
    width: u32,
    /// Visible height; a monochrome shape stores its AND mask rows and then its XOR rows.
    height: u32,
    pitch: u32,
    data: Vec<u8>,
}

/// Draws the pointer with its top-left at (x, y) into a tightly packed BGRA image,
/// clipping whatever falls outside.
fn draw_pointer(image: &mut [u8], width: u32, height: u32, shape: &PointerShape, x: i32, y: i32) {
    let pitch = shape.pitch as usize;
    for row in 0..shape.height as i32 {
        let ty = y + row;
        if ty < 0 || ty >= height as i32 {
            continue;
        }
        for col in 0..shape.width as i32 {
            let tx = x + col;
            if tx < 0 || tx >= width as i32 {
                continue;
            }
            let at = (ty as usize * width as usize + tx as usize) * 4;
            let dst = &mut image[at..at + 3];
            let (row, col) = (row as usize, col as usize);
            match shape.kind {
                PointerKind::Color => {
                    let src = &shape.data[row * pitch + col * 4..][..4];
                    let alpha = u32::from(src[3]);
                    for (d, s) in dst.iter_mut().zip(src) {
                        *d = ((u32::from(*s) * alpha + u32::from(*d) * (255 - alpha)) / 255) as u8;
                    }
                }
                PointerKind::MaskedColor => {
                    let src = &shape.data[row * pitch + col * 4..][..4];
                    for (d, s) in dst.iter_mut().zip(src) {
                        *d = if src[3] == 0 { *s } else { *d ^ *s };
                    }
                }
                PointerKind::Monochrome => {
                    let bit =
                        |r: usize| (shape.data[r * pitch + col / 8] >> (7 - col % 8)) & 1 == 1;
                    let and = if bit(row) { 0xFF } else { 0 };
                    let xor = if bit(row + shape.height as usize) {
                        0xFF
                    } else {
                        0
                    };
                    for d in dst.iter_mut() {
                        *d = (*d & and) ^ xor;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_interval_caps_printwindow_and_clamps_rates() {
        let window = WindowGrabber::new(HWND(std::ptr::null_mut()));
        assert_eq!(frame_interval(10, u32::MAX), Duration::from_millis(100));
        assert_eq!(frame_interval(60, u32::MAX), Duration::from_micros(16_666));
        assert_eq!(
            frame_interval(60, window.max_fps()),
            Duration::from_micros(33_333)
        );
        assert_eq!(frame_interval(0, u32::MAX), Duration::from_secs(1));
    }

    // DXGI allows one duplication of an output per process, and one real capture at a time
    // keeps the frame counts honest.
    static SCREEN: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Captures a 640x480 crop of the real primary screen for a second at 30 fps.
    fn capture_crop(max_height: u32, expected: (u32, u32)) {
        let _screen = SCREEN.lock().unwrap_or_else(|e| e.into_inner());
        let running = Arc::new(AtomicBool::new(true));
        let stop = running.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(1));
            stop.store(false, Ordering::Release);
        });
        let (mut frames, mut lit) = (0, false);
        let source = VideoSource::Screen {
            left: 0,
            top: 0,
            width: 640,
            height: 480,
        };
        capture_video_loop(
            source,
            Arc::new(AtomicU32::new(30)),
            Arc::new(AtomicU32::new(max_height)),
            running,
            Arc::new(VideoMetrics::default()),
            |f| {
                let (w, h) = expected;
                assert_eq!(
                    (f.width, f.height, f.data.len()),
                    (w, h, (w * h * 4) as usize)
                );
                frames += 1;
                lit |= f.data.chunks(4).any(|px| px[..3] != [0, 0, 0]);
                true
            },
        )
        .unwrap();
        // A still screen yields one desktop image, read a tick after its copy, then repeats.
        assert!(frames > 10, "{frames} frames");
        assert!(lit, "all black");
    }

    #[test]
    #[ignore = "captures the real primary screen"]
    fn captures_a_crop_of_the_primary_screen() {
        capture_crop(u32::MAX, (640, 480));
    }

    #[test]
    #[ignore = "captures the real primary screen"]
    fn scales_a_crop_of_the_primary_screen_on_the_gpu() {
        capture_crop(240, (320, 240));
    }

    /// Captures a real window for two seconds at 60 fps; returns (frames, largest frame size).
    fn capture_window(hwnd: HWND, max_height: u32) -> (u32, (u32, u32)) {
        let _screen = SCREEN.lock().unwrap_or_else(|e| e.into_inner());
        let running = Arc::new(AtomicBool::new(true));
        let stop = running.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(2));
            stop.store(false, Ordering::Release);
        });
        let (mut frames, mut lit, mut largest) = (0, false, (0, 0));
        capture_video_loop(
            VideoSource::Window(hwnd),
            Arc::new(AtomicU32::new(60)),
            Arc::new(AtomicU32::new(max_height)),
            running,
            Arc::new(VideoMetrics::default()),
            |f| {
                assert_eq!(f.data.len(), (f.width * f.height * 4) as usize);
                assert_eq!((f.width % 2, f.height % 2), (0, 0), "odd size");
                assert!(f.height <= max_height, "{} tall", f.height);
                frames += 1;
                lit |= f.data.chunks(4).any(|px| px[..3] != [0, 0, 0]);
                largest = largest.max((f.width, f.height));
                true
            },
        )
        .unwrap();
        assert!(lit, "all black");
        (frames, largest)
    }

    #[test]
    #[ignore = "captures a real window"]
    fn captures_a_real_window_through_wgc() {
        let hwnd = crate::sources::enumerate_sources()
            .into_iter()
            .filter(|s| s.kind == crate::sources::SourceKind::Window)
            .map(|s| HWND(s.hwnd as usize as *mut _))
            .find(|&hwnd| unsafe { !IsIconic(hwnd).as_bool() })
            .expect("no open window to capture");
        let (frames, (w, h)) = capture_window(hwnd, u32::MAX);
        // PrintWindow is capped at 30 fps; startup (device creation) takes up to ~0.5 s.
        // More than its 60 frames in two seconds means WGC ran.
        assert!(frames > 60, "{frames} frames");
        assert!(w > 2 && h > 2, "{w}x{h}");
        let (_, (_, scaled_h)) = capture_window(hwnd, 120);
        assert!(scaled_h <= 120 && scaled_h < h, "scaled to {scaled_h}");
    }


    #[test]
    fn scaled_keeps_aspect_and_even_sizes_and_never_grows() {
        assert_eq!(scaled(3840, 2160, 1080), (1920, 1080));
        assert_eq!(scaled(2560, 1440, 1080), (1920, 1080));
        assert_eq!(
            scaled(1282, 911, 720),
            (1012, 720),
            "1013.2 wide rounds down to even"
        );
        assert_eq!(scaled(1920, 1080, 1080), (1920, 1080));
        assert_eq!(scaled(1280, 720, u32::MAX), (1280, 720));
    }

    #[test]
    fn frame_dimensions_are_even() {
        assert_eq!(even(1921), 1920);
        assert_eq!(even(1080), 1080);
        assert_eq!(even(1), 2);
    }

    fn pixel(image: &[u8], width: u32, x: u32, y: u32) -> [u8; 3] {
        let at = ((y * width + x) * 4) as usize;
        [image[at], image[at + 1], image[at + 2]]
    }

    #[test]
    fn color_pointer_blends_by_alpha_and_clips() {
        let mut image = vec![100u8; 4 * 4 * 4];
        let shape = PointerShape {
            kind: PointerKind::Color,
            width: 2,
            height: 2,
            pitch: 8,
            // Opaque white, half-transparent black, then fully transparent.
            data: [
                [255, 255, 255, 255],
                [0, 0, 0, 128],
                [9, 9, 9, 0],
                [9, 9, 9, 0],
            ]
            .concat(),
        };
        draw_pointer(&mut image, 4, 4, &shape, -1, 3);
        // Only the right half of the top row lands inside, at (0, 3).
        assert_eq!(pixel(&image, 4, 0, 3), [49, 49, 49]);
        assert_eq!(pixel(&image, 4, 1, 3), [100, 100, 100]);
        draw_pointer(&mut image, 4, 4, &shape, 2, 0);
        assert_eq!(pixel(&image, 4, 2, 0), [255, 255, 255]);
        assert_eq!(pixel(&image, 4, 2, 1), [100, 100, 100]);
    }

    #[test]
    fn monochrome_pointer_applies_and_then_xor_masks() {
        let mut image = vec![0x40u8; 4 * 4];
        let shape = PointerShape {
            kind: PointerKind::Monochrome,
            width: 4,
            height: 1,
            pitch: 1,
            // AND row 0b0011: keep pixels 2-3. XOR row 0b0101: flip pixels 1 and 3.
            data: vec![0b0011_0000, 0b0101_0000],
        };
        draw_pointer(&mut image, 4, 1, &shape, 0, 0);
        assert_eq!(pixel(&image, 4, 0, 0), [0, 0, 0], "black");
        assert_eq!(pixel(&image, 4, 1, 0), [255, 255, 255], "white");
        assert_eq!(pixel(&image, 4, 2, 0), [0x40; 3], "transparent");
        assert_eq!(pixel(&image, 4, 3, 0), [0xBF; 3], "inverted");
    }

    #[test]
    fn masked_color_pointer_replaces_or_xors() {
        let mut image = vec![0x0Fu8; 2 * 4];
        let shape = PointerShape {
            kind: PointerKind::MaskedColor,
            width: 2,
            height: 1,
            pitch: 8,
            data: [[1, 2, 3, 0], [0xF0, 0xF0, 0xF0, 0xFF]].concat(),
        };
        draw_pointer(&mut image, 2, 1, &shape, 0, 0);
        assert_eq!(pixel(&image, 2, 0, 0), [1, 2, 3]);
        assert_eq!(pixel(&image, 2, 1, 0), [0xFF; 3]);
    }
}
