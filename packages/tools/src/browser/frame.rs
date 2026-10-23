//! The frame both kinds of window are built in, the browser's and the panels': a native window
//! that never takes the keyboard, with its pages filling it. On macOS a shown window is glass:
//! the window is clear, and a native material behind its titlebar and pages samples the desktop.
//! A panel's page is clear too, over a coat of its ground, so the titlebar and the page are one
//! sheet; the browser's pages stay opaque, so what the agent sees in a screenshot never changes.
//! Elsewhere, and when the person reduces transparency, the windows are ordinary ones.

use std::rc::Rc;

use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::WindowEvent;
use tao::event_loop::EventLoopWindowTarget;
use tao::window::{Theme, Window, WindowBuilder};
use wry::WebView;

use super::host::Cmd;
use super::platform::{conceal, keep_rendering};

/// A colour as red, green and blue.
pub(super) type Rgb = [u8; 3];

/// The stylesheet a panel's page is dressed in: its ground, painted once, for each look.
pub(super) const CSS: &str = include_str!("frame.css");
/// The system's grounds, when a page has none of its own.
pub(super) const DARK: Rgb = [0x1e, 0x1e, 0x1e];
pub(super) const LIGHT: Rgb = [0xff, 0xff, 0xff];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Look {
    /// Clear, on a native material (macOS).
    Glass,
    Opaque,
    /// A headless window: invisible, and still rendering.
    Hidden,
}

/// How a window looks: hidden when headless, else glass where there is a material for it and
/// the person has not reduced transparency (`glass`).
pub(super) fn look(headless: bool, glass: bool) -> Look {
    match (headless, glass && cfg!(target_os = "macos")) {
        (true, _) => Look::Hidden,
        (false, true) => Look::Glass,
        (false, false) => Look::Opaque,
    }
}

/// Whether the person has not asked for less transparency.
pub(super) fn glass_allowed() -> bool {
    #[cfg(target_os = "macos")]
    return !native::reduced();
    #[cfg(not(target_os = "macos"))]
    false
}

/// A page's bounds in a window of `size`, `x` points from its left edge.
pub(super) fn fill(size: LogicalSize<f64>, x: f64) -> wry::Rect {
    use wry::dpi::{LogicalPosition as At, LogicalSize as Size};
    wry::Rect {
        position: At::new(x, 0.0).into(),
        size: Size::new(size.width, size.height).into(),
    }
}

/// Whether `event` changes the size the pages must fill.
pub(super) fn relayout(event: &WindowEvent) -> bool {
    matches!(
        event,
        WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
    )
}

/// Whether a ground reads as dark: its luma below the middle.
pub(super) fn dark([red, green, blue]: Rgb) -> bool {
    0.2126 * f64::from(red) + 0.7152 * f64::from(green) + 0.0722 * f64::from(blue) < 127.5
}

/// The material for an appearance (NSVisualEffectMaterial): HUD, which is dark, under a dark
/// ground; under-window background, which reads light, under a light one.
pub(super) fn material(dark: bool) -> isize {
    if dark { 13 } else { 21 }
}

/// The coat over the glass: the page's ground, else the system's, and how much of it covers.
pub(super) fn coat(ground: Option<Rgb>, system_dark: bool) -> (Rgb, f64) {
    let ground = ground.unwrap_or(if system_dark { DARK } else { LIGHT });
    (ground, if dark(ground) { 0.85 } else { 0.92 })
}

/// `page` in the frame's stylesheet, marked as glass when it is.
pub(super) fn dress(page: &str, look: Look) -> String {
    let page = page.replacen("<head>", &format!("<head><style>{CSS}</style>"), 1);
    match look {
        Look::Glass => page.replacen("<html", "<html data-glass", 1),
        _ => page,
    }
}

/// The size a page asks its window to be (`[width, height]`, as its script answers), rounded
/// up and kept to a window's size. The page's answer is read as data, and only as two numbers.
pub(super) fn shape(answer: &str) -> Option<(f64, f64)> {
    let size: [f64; 2] = serde_json::from_str(answer).ok()?;
    let side =
        |value: f64, low: f64, high: f64| value.is_finite().then(|| value.ceil().clamp(low, high));
    Some((side(size[0], 320.0, 2400.0)?, side(size[1], 240.0, 1600.0)?))
}

/// A window as built here, and its glass.
pub(super) struct Frame {
    pub native: Rc<Window>,
    look: Look,
    /// Whether the glass is coated, and in the page's own ground when it has one.
    coat: Option<Option<Rgb>>,
    #[cfg(target_os = "macos")]
    glass: Option<native::Glass>,
}

impl Frame {
    /// A window titled `title` with pages of `size`, at `at` when given. A coated one (`coat`)
    /// is a panel's, in its page's ground or the system's.
    pub fn build(
        target: &EventLoopWindowTarget<Cmd>,
        (title, size, at): (&str, LogicalSize<f64>, Option<LogicalPosition<f64>>),
        look: Look,
        coat: Option<Option<Rgb>>,
    ) -> Result<Self, String> {
        let theme = coat.flatten().map(|ground| match dark(ground) {
            true => Theme::Dark,
            false => Theme::Light,
        });
        let mut builder = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(size)
            .with_focused(false)
            .with_theme(theme);
        if let Some(at) = at {
            builder = builder.with_position(at);
        }
        builder = match look {
            // Shown only once concealed, so it never flashes on screen.
            Look::Hidden => conceal(builder.with_decorations(false).with_visible(false)),
            Look::Glass => native::clear(builder),
            Look::Opaque => builder,
        };
        let native = builder.build(target).map_err(|error| error.to_string())?;
        Ok(Self {
            native: Rc::new(native),
            look,
            coat,
            #[cfg(target_os = "macos")]
            glass: None,
        })
    }

    /// Show the window with its first page, without taking the keyboard, then its glass.
    pub fn show(&mut self, first: &WebView) {
        keep_rendering(first, self.look == Look::Hidden);
        #[cfg(not(target_os = "macos"))]
        if self.look == Look::Hidden {
            self.native.set_visible(true);
        }
        #[cfg(target_os = "macos")]
        if self.look == Look::Glass {
            self.glass = native::Glass::under(&self.native, self.coat.is_some());
            self.retint();
        }
    }

    /// Tint the glass for the system's appearance now, unless its ground is the page's own.
    pub fn retint(&self) {
        #[cfg(target_os = "macos")]
        if let Some(glass) = &self.glass {
            let system = self.native.theme() == Theme::Dark;
            let ground = self.coat.flatten();
            let (colour, alpha) = coat(ground, system);
            glass.tint(material(ground.map_or(system, dark)), colour, alpha);
        }
    }

    /// Take `size`, and keep its shape as the person resizes the window (macOS; elsewhere the
    /// page centres what is left over).
    pub fn reshape(&self, size: LogicalSize<f64>) {
        self.native.set_inner_size(size);
        #[cfg(target_os = "macos")]
        native::keep_shape(&self.native, size.width, size.height);
    }
}

/// For checks: the materials behind `window`, the first one's state, whether the window is
/// opaque, and the shape its resizing keeps (width over height; 0 when none).
pub(super) fn dressed(window: &Window) -> (usize, isize, bool, f64) {
    #[cfg(target_os = "macos")]
    return native::dressed(window);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        (0, -1, true, 0.0)
    }
}

#[cfg(target_os = "macos")]
#[path = "frame_macos.rs"]
mod native;

#[cfg(not(target_os = "macos"))]
mod native {
    pub fn clear(builder: tao::window::WindowBuilder) -> tao::window::WindowBuilder {
        builder
    }
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
