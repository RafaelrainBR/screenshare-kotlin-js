//! Short enter and leave animations, like the web's ui.css motion: an instant and a duration
//! per animation, eased out, and off when Windows' "Show animations" is off.
use std::time::{Duration, Instant};

use iced::{
    Color, Element, Length,
    widget::{container, space, stack},
};

pub const ENTER: Duration = Duration::from_millis(180);
pub const LEAVE: Duration = Duration::from_millis(150);
pub const POP: Duration = Duration::from_millis(120);
pub const SLIDE: Duration = Duration::from_millis(200);

/// How far an animation that started at `since` is, 0 to 1, eased out.
pub fn progress(since: Instant, duration: Duration) -> f32 {
    if reduced() {
        return 1.0;
    }
    ease_out(since.elapsed().as_secs_f32() / duration.as_secs_f32())
}

/// Still moving: the app ticks at 60 Hz meanwhile.
pub fn running(since: Option<Instant>, duration: Duration) -> bool {
    !reduced() && since.is_some_and(|at| at.elapsed() < duration)
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// `content` under a layer of `color` at `alpha`: a group of widgets fading in from (or out to)
/// the background behind it, since iced has no opacity for a whole subtree.
pub fn veil<'a, M: 'a>(content: Element<'a, M>, color: Color, alpha: f32, radius: f32) -> Element<'a, M> {
    if alpha <= 0.0 {
        return content;
    }
    let layer = container(space()).width(Length::Fill).height(Length::Fill).style(move |_| container::Style {
        background: Some(Color { a: color.a * alpha.min(1.0), ..color }.into()),
        border: iced::Border { radius: radius.into(), ..Default::default() },
        ..Default::default()
    });
    // Sized like the content: a Fill content (a video) keeps filling, a shrinking one keeps its box.
    // A max-width panel whose children Fill would stretch to the window: veil inside it instead.
    let size = content.as_widget().size();
    stack![content, layer].width(size.width).height(size.height).into()
}

/// 1 → 0.25 → 1 every 1.2 s for a "still trying" dot; steady when animations are off.
pub fn pulse() -> f32 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    if reduced() {
        return 1.0;
    }
    let turn = START.get_or_init(Instant::now).elapsed().as_secs_f32() / 1.2;
    0.625 + 0.375 * (turn * std::f32::consts::TAU).cos()
}

/// `a` to `b` by `t`.
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color { r: a.r + (b.r - a.r) * t, g: a.g + (b.g - a.g) * t, b: a.b + (b.b - a.b) * t, a: a.a + (b.a - a.a) * t }
}

/// Windows' Settings > Accessibility > Visual effects > "Animation effects".
#[cfg(windows)]
fn reduced() -> bool {
    static REDUCED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *REDUCED.get_or_init(|| {
        use windows::Win32::{
            Foundation::TRUE,
            UI::WindowsAndMessaging::{SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW},
        };
        let mut on = TRUE;
        // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL to the pointer it is given.
        let read = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some((&raw mut on).cast()), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)) };
        read.is_ok() && !on.as_bool()
    })
}

/// shortcut: always animates on macOS; read accessibilityDisplayShouldReduceMotion if asked.
#[cfg(not(windows))]
fn reduced() -> bool {
    false
}

#[test]
fn easing_starts_fast_and_lands_on_one() {
    assert_eq!(ease_out(0.0), 0.0);
    assert!(ease_out(0.5) > 0.8);
    assert_eq!(ease_out(1.0), 1.0);
    assert_eq!(ease_out(2.0), 1.0);
}
