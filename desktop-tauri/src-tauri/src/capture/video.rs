//! Native video capture without the WebView picker, sharing bar or capture border:
//! DXGI Desktop Duplication for screens and PrintWindow for windows (Windows 10 path).
//! Frames are BGRA, top-down, tightly packed (stride = width * 4) and paced to the
//! requested frame rate; a busy consumer drops frames instead of queueing them.
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows::core::{Interface, BOOL};
use windows::Win32::Foundation::{HMODULE, HWND, RECT};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication,
    IDXGIResource, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetWindowDC, ReleaseDC,
    SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ,
};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, IsIconic, IsWindow};

#[link(name = "user32")]
unsafe extern "system" {
    fn PrintWindow(hwnd: HWND, hdc_blt: HDC, flags: u32) -> BOOL;
}
const PW_RENDERFULLCONTENT: u32 = 2;

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
pub fn capture_video_loop(
    source: VideoSource,
    fps: u32,
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
        VideoSource::Window(hwnd) => Box::new(WindowGrabber::new(hwnd)),
    };
    let interval = Duration::from_micros(1_000_000 / u64::from(fps.clamp(1, 120)));
    let start = Instant::now();
    let mut next = start;
    while running.load(Ordering::Acquire) {
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
        let grabbed = grabber.grab(&metrics)?;
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

fn even(value: u32) -> u32 {
    (value & !1).max(2)
}

/// Width, height and BGRA pixels of the latest image.
type Grab<'a> = Result<Option<(u32, u32, &'a [u8])>, String>;

trait Grabber {
    /// Returns the latest image (repeating the previous one when nothing changed),
    /// or None when there is nothing to send yet.
    fn grab(&mut self, metrics: &VideoMetrics) -> Grab<'_>;
}

struct DxgiGrabber {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    output: IDXGIOutput1,
    duplication: Option<IDXGIOutputDuplication>,
    staging: Option<ID3D11Texture2D>,
    // Crop inside the output, in output pixels.
    crop: (u32, u32, u32, u32),
    buffer: Vec<u8>,
    has_frame: bool,
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
                        device,
                        context,
                        output,
                        duplication: None,
                        staging: None,
                        crop: (crop_x, crop_y, width, height),
                        buffer: vec![0; (width * height * 4) as usize],
                        has_frame: false,
                    });
                }
            }
            Err("screen-not-found".into())
        }
    }

    fn acquire(&mut self) -> Result<bool, String> {
        unsafe {
            if self.duplication.is_none() {
                // Fails transiently on the secure desktop (UAC, lock screen): retry next tick.
                self.duplication = self.output.DuplicateOutput(&self.device).ok();
            }
            let Some(duplication) = &self.duplication else {
                return Ok(false);
            };
            let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource: Option<IDXGIResource> = None;
            match duplication.AcquireNextFrame(0, &mut info, &mut resource) {
                Ok(()) => {}
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok(false),
                Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST => {
                    self.duplication = None;
                    return Ok(false);
                }
                Err(_) => return Err("dxgi-capture-failed".into()),
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
                if self.staging.is_none() {
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    texture.GetDesc(&mut desc);
                    desc.Usage = D3D11_USAGE_STAGING;
                    desc.BindFlags = 0;
                    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
                    desc.MiscFlags = 0;
                    let mut staging = None;
                    self.device
                        .CreateTexture2D(&desc, None, Some(&mut staging))
                        .map_err(|_| "dxgi-capture-failed")?;
                    self.staging = staging;
                }
                self.context
                    .CopyResource(self.staging.as_ref().unwrap(), &texture);
                Ok::<bool, String>(true)
            })();
            let _ = duplication.ReleaseFrame();
            result
        }
    }

    fn read_staging(&mut self) -> Result<(), String> {
        let Some(staging) = &self.staging else {
            return Ok(());
        };
        unsafe {
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(|_| "dxgi-capture-failed")?;
            let (x, y, w, h) = self.crop;
            let row = (w * 4) as usize;
            for line in 0..h as usize {
                let src = (mapped.pData as *const u8)
                    .add((y as usize + line) * mapped.RowPitch as usize + x as usize * 4);
                std::ptr::copy_nonoverlapping(src, self.buffer[line * row..].as_mut_ptr(), row);
            }
            self.context.Unmap(staging, 0);
        }
        Ok(())
    }
}

impl Grabber for DxgiGrabber {
    fn grab(&mut self, metrics: &VideoMetrics) -> Grab<'_> {
        if self.acquire()? {
            metrics.new_frames.fetch_add(1, Ordering::Relaxed);
            self.read_staging()?;
            self.has_frame = true;
        }
        if !self.has_frame {
            return Ok(None);
        }
        let (_, _, w, h) = self.crop;
        Ok(Some((w, h, &self.buffer)))
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

struct WindowGrabber {
    hwnd: HWND,
    dib: Option<Dib>,
    // Visible frame inside the window rect (Windows 10 adds invisible resize borders).
    visible: (u32, u32, u32, u32),
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
            visible: (0, 0, 0, 0),
            buffer: Vec::new(),
            has_frame: false,
        }
    }

    fn ensure_dib(&mut self, width: u32, height: u32) -> Result<(), String> {
        if self
            .dib
            .as_ref()
            .is_some_and(|d| d.width == width && d.height == height)
        {
            return Ok(());
        }
        self.dib = None;
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
            let window_dc = GetWindowDC(Some(self.hwnd));
            if window_dc.0.is_null() {
                return Err("window-capture-failed".into());
            }
            let dc = CreateCompatibleDC(Some(window_dc));
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let bitmap =
                CreateDIBSection(Some(window_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0);
            let _ = ReleaseDC(Some(self.hwnd), window_dc);
            let bitmap = match bitmap {
                Ok(bitmap) if !bits.is_null() && !dc.0.is_null() => bitmap,
                _ => {
                    let _ = DeleteDC(dc);
                    return Err("window-capture-failed".into());
                }
            };
            let old = SelectObject(dc, HGDIOBJ(bitmap.0));
            self.dib = Some(Dib {
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
}

impl Grabber for WindowGrabber {
    fn grab(&mut self, metrics: &VideoMetrics) -> Grab<'_> {
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
                self.ensure_dib(width, height)?;
                let dib = self.dib.as_ref().unwrap();
                if PrintWindow(self.hwnd, dib.dc, PW_RENDERFULLCONTENT).as_bool() {
                    let x = (frame.left - rect.left).clamp(0, width as i32) as u32;
                    let y = (frame.top - rect.top).clamp(0, height as i32) as u32;
                    let w = even(((frame.right - frame.left).max(0) as u32).min(width - x));
                    let h = even(((frame.bottom - frame.top).max(0) as u32).min(height - y));
                    self.visible = (x, y, w, h);
                    let row = (w * 4) as usize;
                    self.buffer.resize(row * h as usize, 0);
                    for line in 0..h as usize {
                        let src = dib
                            .bits
                            .add(((y as usize + line) * width as usize + x as usize) * 4);
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
        let (_, _, w, h) = self.visible;
        Ok(Some((w, h, &self.buffer)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_dimensions_are_even() {
        assert_eq!(even(1921), 1920);
        assert_eq!(even(1080), 1080);
        assert_eq!(even(1), 2);
    }
}
