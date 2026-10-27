//! The strip in the colour of the tab under it, as Safari tints its bar: whatever the tab is (a
//! page, a terminal's screen), its top row of pixels as drawn is read, and where one colour holds
//! most of it the strip takes that colour, its words in the ink that reads on it. A busy top (a
//! picture, a banner) leaves the strip as it is. Nothing of the tab's own code is asked.

/// A pixel, in sRGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// The colour the strip's words and buttons are drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ink {
    Dark,
    Light,
}

/// The share of the pixels one colour must hold for the strip to take it.
const MOST: f64 = 0.7;

/// How far apart, in any channel, two pixels may be and count as one colour (a JPEG's noise, a
/// gradient's step).
const NEAR: u8 = 10;

/// How many pixels are weighed at most.
const SAMPLES: usize = 256;

/// The colour that holds most of `pixels`, near ones counted as one; none when no colour does.
pub(super) fn dominant(pixels: &[Rgb]) -> Option<Rgb> {
    // Each pixel is weighed against every other: a few hundred, evenly picked, say as much.
    let step = pixels.len().div_ceil(SAMPLES).max(1);
    let pixels: Vec<Rgb> = pixels.iter().step_by(step).copied().collect();
    let near = |a: &Rgb, b: &Rgb| {
        a.r.abs_diff(b.r) <= NEAR && a.g.abs_diff(b.g) <= NEAR && a.b.abs_diff(b.b) <= NEAR
    };
    let around = |p: &Rgb| pixels.iter().filter(|q| near(p, q)).count();
    let centre = pixels.iter().max_by_key(|p| around(p))?;
    let held: Vec<&Rgb> = pixels.iter().filter(|q| near(centre, q)).collect();
    if (held.len() as f64) < MOST * pixels.len() as f64 {
        return None;
    }
    let mean = |channel: fn(&Rgb) -> u8| {
        let sum: usize = held.iter().map(|p| usize::from(channel(p))).sum();
        u8::try_from((sum + held.len() / 2) / held.len()).unwrap_or(u8::MAX)
    };
    Some(Rgb {
        r: mean(|p| p.r),
        g: mean(|p| p.g),
        b: mean(|p| p.b),
    })
}

/// The ink with the greater contrast on `ground` (WCAG's relative luminance).
pub(super) fn ink(ground: Rgb) -> Ink {
    let linear = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance =
        0.2126 * linear(ground.r) + 0.7152 * linear(ground.g) + 0.0722 * linear(ground.b);
    // Black's contrast (L + 0.05) / 0.05 against white's 1.05 / (L + 0.05): equal at L ≈ 0.179.
    if luminance > 0.179 {
        Ink::Dark
    } else {
        Ink::Light
    }
}

/// What a tab's top was when last drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Seen {
    /// One colour held it.
    Tinted(Rgb),
    /// None did: the strip keeps its own colours.
    Busy,
}

impl Seen {
    pub(super) fn from(ground: Option<Rgb>) -> Self {
        ground.map_or(Self::Busy, Self::Tinted)
    }

    pub(super) fn ground(self) -> Option<Rgb> {
        match self {
            Self::Tinted(ground) => Some(ground),
            Self::Busy => None,
        }
    }
}

/// The tabs' colours as last drawn, so a tab switched to is tinted at once, and the tabs being
/// drawn, so a tab is drawn once at a time however often the strip is refreshed.
pub(super) struct Memory<K> {
    known: Vec<(K, Seen)>,
    drawing: Vec<K>,
}

impl<K> Default for Memory<K> {
    fn default() -> Self {
        Self {
            known: vec![],
            drawing: vec![],
        }
    }
}

/// How many tabs' colours are kept: more than a window holds.
const KEPT: usize = 32;

impl<K: PartialEq> Memory<K> {
    /// The colour `tab` had when last drawn, if it was.
    pub(super) fn recall(&self, tab: &K) -> Option<Seen> {
        self.known
            .iter()
            .find(|(key, _)| key == tab)
            .map(|&(_, seen)| seen)
    }

    /// Whether to draw `tab` now: not while a picture of it is on its way.
    pub(super) fn start(&mut self, tab: K) -> bool {
        if self.drawing.contains(&tab) {
            return false;
        }
        self.drawing.push(tab);
        true
    }

    /// `tab` drawn: its colour is `ground`.
    pub(super) fn remember(&mut self, tab: K, ground: Option<Rgb>) {
        self.drawing.retain(|key| *key != tab);
        self.known.retain(|(key, _)| *key != tab);
        self.known.push((tab, Seen::from(ground)));
        if self.known.len() > KEPT {
            self.known.remove(0);
        }
    }
}

/// The call that tints the strip with `ground`, or gives it back its own colours.
pub(super) fn script(ground: Option<Rgb>) -> String {
    let Some(ground) = ground else {
        return "T(null)".into();
    };
    let ink = match ink(ground) {
        Ink::Dark => "dark",
        Ink::Light => "light",
    };
    let hex = format!("#{:02x}{:02x}{:02x}", ground.r, ground.g, ground.b);
    format!(r#"T({{"ground":"{hex}","ink":"{ink}"}})"#)
}

#[cfg(test)]
#[path = "tint_tests.rs"]
mod tests;
