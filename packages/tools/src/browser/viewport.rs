//! The page's size. The window opens at desktop size; the toolbar resizes it (layout only: the
//! user agent stays the one the window opened with).

#[derive(Debug, Clone, PartialEq)]
pub struct Viewport {
    pub width: f64,
    pub height: f64,
}

impl Viewport {
    pub const DESKTOP: Viewport = Viewport {
        width: 1280.0,
        height: 860.0,
    };
}
