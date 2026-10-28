//! Short enter and leave animations, like the web's ui.css motion: an instant and a duration
//! per animation, eased out, and off when Windows' "Show animations" is off.
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    hash::Hash,
    time::{Duration, Instant},
};

use iced::{
    Color, Element, Event, Length, Rectangle, Size, Transformation, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    widget::{container, space, stack},
};

pub const ENTER: Duration = Duration::from_millis(180);
pub const LEAVE: Duration = Duration::from_millis(150);
pub const POP: Duration = Duration::from_millis(120);
pub const SLIDE: Duration = Duration::from_millis(200);
pub const FLIP: Duration = Duration::from_millis(220);
pub const ICON: Duration = Duration::from_millis(140);
pub const ROOM: Duration = Duration::from_millis(200);
pub const HOVER: Duration = Duration::from_millis(150);

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

thread_local! {
    // Per control: hovered when last drawn, and since when it changed (None once settled).
    static HOVERS: RefCell<HashMap<String, (bool, Option<Instant>)>> = RefCell::default();
}

/// 0 → 1 over HOVER as `key` turns hovered, and back as it leaves (the web's background .15s).
/// iced keeps no hover time, so this runs in the style closure, which sees the status on every draw.
pub fn hover(key: &str, hovered: bool) -> f32 {
    HOVERS.with(|hovers| {
        let mut hovers = hovers.borrow_mut();
        let entry = hovers.entry(key.to_owned()).or_insert((hovered, None));
        if entry.0 != hovered {
            *entry = (hovered, Some(Instant::now()));
        }
        let t = entry.1.map_or(1.0, |at| progress(at, HOVER));
        if hovered { t } else { 1.0 - t }
    })
}

/// A hover color still changing: the app ticks at 60 Hz meanwhile.
pub fn hovers_running() -> bool {
    HOVERS.with(|hovers| hovers.borrow().values().any(|(_, at)| running(*at, HOVER)))
}

/// `a` to `b` by `t`.
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color { r: a.r + (b.r - a.r) * t, g: a.g + (b.g - a.g) * t, b: a.b + (b.b - a.b) * t, a: a.a + (b.a - a.a) * t }
}

/// Shrinks `content` to `scale` about its center while the left button holds it, like the web :active.
pub fn press<'a, M: 'a>(content: impl Into<Element<'a, M>>, scale: f32) -> Element<'a, M> {
    Moved { content: content.into(), place: Box::new(move |bounds, held| if held && !reduced() { scaled(bounds, scale) } else { bounds }) }.into()
}

/// Where each keyed tile was last drawn, so a new arrangement slides it from there (the web Motion.flip).
pub struct Flips<K> {
    arrangement: Cell<u64>,
    slots: RefCell<HashMap<K, Slot>>,
}

impl<K> Default for Flips<K> {
    fn default() -> Self {
        Self { arrangement: Cell::new(0), slots: RefCell::default() }
    }
}

struct Slot {
    at: Rectangle,
    from: Rectangle,
    since: Option<Instant>,
    arrangement: u64,
}

impl Slot {
    /// Between `from` and `at`, at `at`'s aspect: scaled to fit inside `from` (one scale for both
    /// axes, so a tall grid cell going to the 16:9 rail does not stretch) and eased to full size.
    fn drawn(&self) -> Rectangle {
        let Some(since) = self.since else { return self.at };
        let t = progress(since, FLIP);
        if t >= 1.0 || self.at.width < 1.0 || self.at.height < 1.0 {
            return self.at;
        }
        let start = (self.from.width / self.at.width).min(self.from.height / self.at.height);
        let (from, to) = (self.from.center(), self.at.center());
        let center = from + (to - from) * t;
        scaled(Rectangle { x: center.x - self.at.width / 2.0, y: center.y - self.at.height / 2.0, ..self.at }, start + (1.0 - start) * t)
    }
}

impl<K: Hash + Eq + Clone> Flips<K> {
    /// The tiles on the stage and a hash of how they are arranged: when it changes, the tiles
    /// that land somewhere else slide there. Window resizes and the chat sliding change nothing.
    pub fn arrange(&self, keys: &[K], arrangement: u64) {
        if self.arrangement.replace(arrangement) != arrangement {
            self.slots.borrow_mut().retain(|key, _| keys.contains(key));
        }
    }

    pub fn running(&self) -> bool {
        self.slots.borrow().values().any(|slot| running(slot.since, FLIP))
    }

    fn place(&self, key: &K, bounds: Rectangle) -> Rectangle {
        let arrangement = self.arrangement.get();
        let mut slots = self.slots.borrow_mut();
        let slot = slots.entry(key.clone()).or_insert(Slot { at: bounds, from: bounds, since: None, arrangement });
        if slot.arrangement != arrangement {
            if slot.at != bounds && !reduced() {
                slot.from = slot.drawn();
                slot.since = Some(Instant::now());
            }
            slot.arrangement = arrangement;
        }
        slot.at = bounds;
        slot.drawn()
    }
}

/// `content` slides from where `key` was drawn before the last [`Flips::arrange`].
pub fn flip<'a, M: 'a, K: Hash + Eq + Clone + 'a>(content: impl Into<Element<'a, M>>, flips: &'a Flips<K>, key: K) -> Element<'a, M> {
    Moved { content: content.into(), place: Box::new(move |bounds, _| flips.place(&key, bounds)) }.into()
}

fn scaled(bounds: Rectangle, scale: f32) -> Rectangle {
    let (width, height) = (bounds.width * scale, bounds.height * scale);
    Rectangle { x: bounds.center_x() - width / 2.0, y: bounds.center_y() - height / 2.0, width, height }
}

/// Draws `content` in the rectangle `place` gives for its layout bounds (and whether the left button
/// holds it). Only the drawing moves: clicks land where the layout put it, which is where it ends up.
struct Moved<'a, M> {
    content: Element<'a, M>,
    place: Box<dyn Fn(Rectangle, bool) -> Rectangle + 'a>,
}

#[derive(Default)]
struct Held(bool);

impl<M> Widget<M, iced::Theme, iced::Renderer> for Moved<'_, M> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Held>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Held::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        self.content.as_widget_mut().layout(&mut tree.children[0], renderer, limits)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        // Seen before the content, which captures the press (a button does).
        let held = &mut tree.state.downcast_mut::<Held>().0;
        let now = match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => cursor.is_over(layout.bounds()),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left) | mouse::Event::CursorLeft) => false,
            _ => *held,
        };
        if now != *held {
            *held = now;
            shell.request_redraw();
        }
        self.content.as_widget_mut().update(&mut tree.children[0], event, layout, cursor, renderer, clipboard, shell, viewport);
    }

    fn draw(&self, tree: &Tree, renderer: &mut iced::Renderer, theme: &iced::Theme, style: &renderer::Style, layout: Layout<'_>, cursor: mouse::Cursor, viewport: &Rectangle) {
        use iced::advanced::Renderer as _;
        let bounds = layout.bounds();
        let to = (self.place)(bounds, tree.state.downcast_ref::<Held>().0);
        let content = &tree.children[0];
        if to == bounds || bounds.width < 1.0 {
            return self.content.as_widget().draw(content, renderer, theme, style, layout, cursor, viewport);
        }
        let transformation = Transformation::translate(to.center_x(), to.center_y())
            * Transformation::scale(to.width / bounds.width)
            * Transformation::translate(-bounds.center_x(), -bounds.center_y());
        renderer.with_transformation(transformation, |renderer| {
            self.content.as_widget().draw(content, renderer, theme, style, layout, cursor * transformation.inverse(), &(*viewport * transformation.inverse()));
        });
    }

    fn mouse_interaction(&self, tree: &Tree, layout: Layout<'_>, cursor: mouse::Cursor, viewport: &Rectangle, renderer: &iced::Renderer) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>, renderer: &iced::Renderer, operation: &mut dyn Operation) {
        self.content.as_widget_mut().operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn overlay<'b>(&'b mut self, tree: &'b mut Tree, layout: Layout<'b>, renderer: &iced::Renderer, viewport: &Rectangle, offset: Vector) -> Option<overlay::Element<'b, M, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(&mut tree.children[0], layout, renderer, viewport, offset)
    }
}

impl<'a, M: 'a> From<Moved<'a, M>> for Element<'a, M> {
    fn from(moved: Moved<'a, M>) -> Self {
        Element::new(moved)
    }
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

/// System Settings > Accessibility > Display > "Reduce motion", read once like the Windows one.
#[cfg(not(windows))]
fn reduced() -> bool {
    static REDUCED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *REDUCED.get_or_init(|| {
        let read = std::process::Command::new("defaults").args(["read", "com.apple.universalaccess", "reduceMotion"]).output();
        read.is_ok_and(|out| out.stdout.trim_ascii() == b"1")
    })
}

#[test]
fn a_hover_fades_in_and_out_from_where_it_settled() {
    if reduced() {
        return;
    }
    assert_eq!(hover("test", false), 0.0);
    assert!(hover("test", true) < 0.1);
    assert!(hovers_running());
    std::thread::sleep(HOVER);
    assert_eq!(hover("test", true), 1.0);
    assert!(hover("test", false) > 0.9);
}

#[test]
fn a_moved_tile_starts_inside_its_old_slot_without_stretching() {
    if reduced() {
        return;
    }
    let flips = Flips::<u8>::default();
    let grid = Rectangle { x: 0.0, y: 0.0, width: 600.0, height: 700.0 };
    let rail = Rectangle { x: 100.0, y: 800.0, width: 168.0, height: 94.0 };
    assert_eq!(flips.place(&1, grid), grid);
    flips.arrange(&[1], 1);
    let start = flips.place(&1, rail);
    assert!(flips.running());
    assert!(start.x >= grid.x && start.y >= grid.y && start.x + start.width <= grid.width + 0.01 && start.y + start.height <= grid.height + 0.01);
    assert!((start.width / start.height - rail.width / rail.height).abs() < 0.01);
    // A resize in the same arrangement just follows the layout.
    let flips = Flips::<u8>::default();
    flips.place(&1, grid);
    assert_eq!(flips.place(&1, rail), rail);
}

#[test]
fn easing_starts_fast_and_lands_on_one() {
    assert_eq!(ease_out(0.0), 0.0);
    assert!(ease_out(0.5) > 0.8);
    assert_eq!(ease_out(1.0), 1.0);
    assert_eq!(ease_out(2.0), 1.0);
}
