//! The frame both kinds of window are built in, the browser's and the panels': a native window
//! that never takes the keyboard, with its pages filling it. On macOS a shown window is glass:
//! the window is clear, and a native material behind its titlebar and pages samples the desktop.
//! A panel's page is clear too, over a coat of its ground, so the titlebar and the page are one
//! sheet; the browser's pages stay opaque, so what the agent sees in a screenshot never changes.
//! Elsewhere, and when the person reduces transparency, the windows are ordinary ones.

use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::WindowEvent;
use tao::event_loop::EventLoopWindowTarget;
use tao::window::{Theme, Window, WindowBuilder};
use wry::WebView;

use super::host::Cmd;
use super::platform::{conceal, keep_rendering, keep_tab_rendering};

/// A colour as red, green and blue.
pub(super) type Rgb = [u8; 3];

/// The stylesheet a panel's page is dressed in: its ground, painted once, for each look.
pub(super) const CSS: &str = include_str!("frame.css");
/// The system's grounds, when a page has none of its own.
#[cfg_attr(
    not(any(target_os = "macos", test)),
    expect(
        dead_code,
        reason = "the glass under a window is macOS's only; its tests use it everywhere"
    )
)]
pub(super) const DARK: Rgb = [0x07, 0x07, 0x07];
#[cfg_attr(
    not(any(target_os = "macos", test)),
    expect(
        dead_code,
        reason = "the glass under a window is macOS's only; its tests use it everywhere"
    )
)]
pub(super) const LIGHT: Rgb = [0xf7, 0xf7, 0xf7];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Look {
    /// Clear, on a native material (macOS).
    Glass,
    Opaque,
    /// A headless window: invisible, and still rendering.
    Hidden,
}

/// Whether a window may be clear: the system has a material for it and the person has not
/// asked for less transparency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Transparency {
    Allowed,
    Reduced,
}

/// How a window looks: hidden when headless, else glass where `transparency` allows it.
pub(super) fn look(headless: bool, transparency: Transparency) -> Look {
    let glass = transparency == Transparency::Allowed && cfg!(target_os = "macos");
    match (headless, glass) {
        (true, _) => Look::Hidden,
        (false, true) => Look::Glass,
        (false, false) => Look::Opaque,
    }
}

/// Whether the person has not asked for less transparency, on a system with glass.
pub(super) fn transparency() -> Transparency {
    #[cfg(target_os = "macos")]
    if !native::reduced() {
        return Transparency::Allowed;
    }
    Transparency::Reduced
}

/// A page's bounds: `size`, `x` points from the window's left edge and `top` from its top.
#[cfg_attr(
    all(target_os = "linux", not(test)),
    expect(
        dead_code,
        reason = "a page in a GTK box takes its place from the box; its tests use it everywhere"
    )
)]
pub(super) fn fill(size: LogicalSize<f64>, x: f64, top: f64) -> wry::Rect {
    use wry::dpi::{LogicalPosition as At, LogicalSize as Size};
    wry::Rect {
        position: At::new(x, top).into(),
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
#[cfg_attr(
    not(any(target_os = "macos", test)),
    expect(
        dead_code,
        reason = "the glass under a window is macOS's only; its tests use it everywhere"
    )
)]
pub(super) fn material(dark: bool) -> isize {
    if dark { 13 } else { 21 }
}

/// The coat over the glass, and how much of it covers: neutral near-black or near-white
/// (oklch 0.13 and 0.975), never the page's own colour. A terminal's
/// grey-blue ground over the material read as grey; the page's ground, else the system's, only
/// says which appearance.
#[cfg_attr(
    not(any(target_os = "macos", test)),
    expect(
        dead_code,
        reason = "the glass under a window is macOS's only; its tests use it everywhere"
    )
)]
pub(super) fn coat(ground: Option<Rgb>, system_dark: bool) -> (Rgb, f64) {
    match ground.map_or(system_dark, dark) {
        // Thin: the desktop shows through, text still reads.
        true => (DARK, 0.5),
        false => (LIGHT, 0.6),
    }
}

/// `page` in the frame's stylesheet, marked as glass when it is.
pub(super) fn dress(page: &str, look: Look) -> String {
    let page = page.replacen("<head>", &format!("<head><style>{CSS}</style>"), 1);
    match look {
        Look::Glass => page.replacen("<html", "<html data-glass", 1),
        Look::Opaque | Look::Hidden => page,
    }
}

/// A window as built here, and its glass.
pub(super) struct Frame {
    pub(super) native: Window,
    look: Look,
    /// The page's own ground the glass is coated in, when it has one; else the system's.
    ground: Option<Rgb>,
    #[cfg(target_os = "macos")]
    glass: Option<native::Glass>,
    /// How the window keeps drawing while it is concealed.
    #[cfg(target_os = "macos")]
    rendering: super::nspanel::Rendering,
}

impl Frame {
    /// A window titled `title` with pages of `size`, at `at` when given, its glass coated in
    /// `ground` (a panel's page's), else in the system's appearance.
    pub(super) fn build(
        target: &EventLoopWindowTarget<Cmd>,
        (title, size, at): (&str, LogicalSize<f64>, Option<LogicalPosition<f64>>),
        look: Look,
        ground: Option<Rgb>,
    ) -> Result<Self, String> {
        let theme = ground.map(|ground| match dark(ground) {
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
        // On macOS, order the window only after its Space and level are configured.
        #[cfg(target_os = "macos")]
        {
            builder = builder.with_visible(false);
        }
        builder = match look {
            // Shown only once concealed, so it never flashes on screen.
            Look::Hidden => conceal(builder.with_decorations(false).with_visible(false)),
            Look::Glass => native::clear(builder),
            Look::Opaque => builder,
        };
        let native = builder.build(target).map_err(|error| error.to_string())?;
        // A panel before any page is built in it: a WebView observes its window (KVO), and a
        // window whose class is swapped afterwards fails to remove the observer.
        #[cfg(target_os = "macos")]
        if look != Look::Hidden {
            super::nspanel::adopt(&native, super::nspanel::Role::Deck);
        }
        Ok(Self {
            native,
            look,
            ground,
            #[cfg(target_os = "macos")]
            glass: None,
            #[cfg(target_os = "macos")]
            rendering: super::nspanel::Rendering::Switched,
        })
    }

    /// Show the window with its first page, its glass, and, where it is shown to the person, as a
    /// panel (macOS) concealed: [`Frame::reveal`] opens it, when the person expands it.
    pub(super) fn show(&mut self, first: &WebView) {
        if self.look == Look::Hidden {
            keep_rendering(first);
            #[cfg(not(target_os = "macos"))]
            self.native.set_visible(true);
            return;
        }
        #[cfg(target_os = "macos")]
        {
            use super::nspanel::Rendering;
            self.rendering = match keep_tab_rendering(first) {
                true => Rendering::Switched,
                false => Rendering::Floating,
            };
            self.conceal();
            if self.look == Look::Glass {
                self.glass = native::Glass::under(&self.native);
                self.retint();
            }
        }
        #[cfg(not(target_os = "macos"))]
        let _ = keep_tab_rendering(first);
    }

    /// The window on screen, for the person to use (macOS: it comes over the Space they are in
    /// without taking the keyboard; elsewhere it is always shown).
    pub(super) fn reveal(&self) {
        // Elsewhere the window is an ordinary one, always as it is.
        #[cfg(not(target_os = "macos"))]
        let _ = &self.native;
        #[cfg(target_os = "macos")]
        super::nspanel::reveal(&self.native, self.rendering);
    }

    /// The window out of sight, at the size it has, clear and click-through, still drawing.
    pub(super) fn conceal(&self) {
        // Elsewhere the window is an ordinary one, always as it is.
        #[cfg(not(target_os = "macos"))]
        let _ = &self.native;
        #[cfg(target_os = "macos")]
        super::nspanel::conceal(&self.native, self.rendering);
    }

    /// Tint the glass for the system's appearance now, unless its ground is the page's own.
    #[cfg_attr(
        not(target_os = "macos"),
        expect(
            clippy::unused_self,
            reason = "the glass under a window is macOS's only"
        )
    )]
    pub(super) fn retint(&self) {
        #[cfg(target_os = "macos")]
        if let Some(glass) = &self.glass {
            let system = self.native.theme() == Theme::Dark;
            let ground = self.ground;
            let (colour, alpha) = coat(ground, system);
            glass.tint(material(ground.map_or(system, dark)), colour, alpha);
        }
    }

    /// Take `ground` as the window's own (a terminal's colours), its appearance with it.
    pub(super) fn ground(&mut self, ground: Rgb) {
        self.ground = Some(ground);
        let theme = if dark(ground) {
            Theme::Dark
        } else {
            Theme::Light
        };
        self.native.set_theme(Some(theme));
        self.retint();
    }
}

/// For checks: the materials behind `window`, the first one's state, whether the window is
/// opaque, the shape its resizing keeps (width over height; 0 when none), and whether it is
/// coated.
pub(super) fn dressed(window: &Window) -> (usize, isize, bool, f64, bool) {
    #[cfg(target_os = "macos")]
    return native::dressed(window);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        (0, -1, true, 0.0, false)
    }
}

#[cfg(target_os = "macos")]
#[path = "frame_macos.rs"]
mod native;

#[cfg(not(target_os = "macos"))]
mod native {
    pub(super) fn clear(builder: tao::window::WindowBuilder) -> tao::window::WindowBuilder {
        builder
    }
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
