use serde::Serialize;
use std::sync::Mutex;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, GetWindowDC,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HDC, HGDIOBJ,
    SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, IsWindow};

#[link(name = "user32")]
unsafe extern "system" {
    fn PrintWindow(hwnd: HWND, hdc_blt: HDC, flags: u32) -> BOOL;
}
pub const MAX_THUMB_DIM: u32 = 320;
static THUMBNAIL_CAPTURE_LOCK: Mutex<()> = Mutex::new(());
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Thumbnail {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}
/// Captures on-screen window pixels into a bounded top-down BGRA DIB. The
/// caller requests this lazily, so inventory enumeration never transfers image data.
pub fn capture_thumbnail(hwnd: HWND, max_dim: u32) -> Result<Option<Thumbnail>, String> {
    // Several tiles request previews together. Serialize the native captures so
    // full-size intermediate DIBs do not multiply memory usage.
    let _capture_guard = THUMBNAIL_CAPTURE_LOCK
        .lock()
        .map_err(|_| "thumbnail-lock-failed")?;
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return Ok(None);
    }
    let mut rect = RECT::default();
    unsafe {
        let _ = GetWindowRect(hwnd, &mut rect);
    }
    let source_w = (rect.right - rect.left).max(0) as u32;
    let source_h = (rect.bottom - rect.top).max(0) as u32;
    if source_w == 0 || source_h == 0 {
        return Ok(None);
    }
    let (width, height) = fit(source_w, source_h, max_dim.clamp(1, MAX_THUMB_DIM));
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: source_w as i32,
            biHeight: -(source_h as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: source_w * source_h * 4,
            ..Default::default()
        },
        bmiColors: [Default::default()],
    };
    unsafe {
        let window_dc = GetWindowDC(Some(hwnd));
        if window_dc.0.is_null() {
            return Ok(None);
        }
        let dc = CreateCompatibleDC(Some(window_dc));
        if dc.0.is_null() {
            let _ = ReleaseDC(Some(hwnd), window_dc);
            return Err("thumbnail-dc-failed".into());
        }
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let bitmap =
            match CreateDIBSection(Some(window_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
                Ok(bitmap) if !bits.is_null() => bitmap,
                _ => {
                    let _ = DeleteDC(dc);
                    let _ = ReleaseDC(Some(hwnd), window_dc);
                    return Err("thumbnail-bitmap-failed".into());
                }
            };
        let old: HGDIOBJ = SelectObject(dc, HGDIOBJ(bitmap.0));
        let rendered = PrintWindow(hwnd, dc, 2).as_bool();
        let _ = ReleaseDC(Some(hwnd), window_dc);

        let source_pixel_count = (source_w * source_h * 4) as usize;
        let print_was_blank = std::slice::from_raw_parts(bits as *const u8, source_pixel_count)
            .chunks_exact(4)
            .all(|pixel| pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0);

        // Some applications report success but return an empty black surface.
        // Fall back to composited desktop pixels for a visible GPU window.
        if !rendered || print_was_blank {
            let screen_dc = GetDC(None);
            if !screen_dc.0.is_null() {
                let _ = BitBlt(
                    dc,
                    0,
                    0,
                    source_w as i32,
                    source_h as i32,
                    Some(screen_dc),
                    rect.left,
                    rect.top,
                    SRCCOPY,
                );
                let _ = ReleaseDC(None, screen_dc);
            }
        }
        let source_data = std::slice::from_raw_parts(bits as *const u8, source_pixel_count);
        let data = resize_bgra(source_data, source_w, source_h, width, height);
        let is_blank = data
            .chunks_exact(4)
            .all(|pixel| pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0);
        SelectObject(dc, old);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(dc);
        if is_blank {
            return Ok(None);
        }
        Ok(Some(Thumbnail {
            width,
            height,
            data,
        }))
    }
}

fn resize_bgra(source: &[u8], source_w: u32, source_h: u32, width: u32, height: u32) -> Vec<u8> {
    let mut output = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        let source_y = y * source_h / height;
        for x in 0..width {
            let source_x = x * source_w / width;
            let source_index = ((source_y * source_w + source_x) * 4) as usize;
            let output_index = ((y * width + x) * 4) as usize;
            output[output_index..output_index + 3]
                .copy_from_slice(&source[source_index..source_index + 3]);
            output[output_index + 3] = 255;
        }
    }
    output
}

pub(crate) fn fit(w: u32, h: u32, max: u32) -> (u32, u32) {
    let scale = (max as f32) / (w.max(h) as f32);
    let s = scale.min(1.0);
    (
        ((w as f32 * s).round() as u32).max(1),
        ((h as f32 * s).round() as u32).max(1),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thumbnail_dimensions_are_bounded() {
        assert_eq!(fit(1920, 1080, MAX_THUMB_DIM), (320, 180));
        assert_eq!(fit(100, 100, MAX_THUMB_DIM), (100, 100));
    }
}
