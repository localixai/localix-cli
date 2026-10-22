//! The browser's own controls, shown beside the page like a real browser's toolbar: back, reload,
//! and the page size (a phone or a tablet, for how a site lays out at that width). They are acted
//! on like any control, by ref; the page never sees them.

use agent::tool::{Element, SelectOption};

pub(crate) const BACK: &str = "back";
pub(crate) const RELOAD: &str = "reload";
pub(crate) const VIEWPORT: &str = "viewport";

/// The sizes the toolbar offers: (value, label, width, height).
const SIZES: [(&str, &str, f64, f64); 3] = [
    ("desktop", "desktop 1280×860", 1280.0, 860.0),
    ("tablet", "tablet 820×1180", 820.0, 1180.0),
    ("mobile", "mobile 390×844", 390.0, 844.0),
];

/// Whether `reference` names a toolbar control rather than something on the page.
pub(crate) fn owns(reference: &str) -> bool {
    [BACK, RELOAD, VIEWPORT].contains(&reference)
}

/// The toolbar's controls, with the size the page has now.
pub(crate) fn controls(size: &str) -> Vec<Element> {
    let button = |reference: &str, name: &str| Element {
        reference: reference.into(),
        role: "button".into(),
        name: name.into(),
        value: None,
        states: vec![],
        editable: false,
        dialog: false,
        options: vec![],
    };
    let label = SIZES
        .iter()
        .find(|(value, ..)| *value == size)
        .map_or(size, |(_, label, ..)| label);
    vec![
        button(BACK, "Back"),
        button(RELOAD, "Reload"),
        Element {
            role: "combobox".into(),
            name: "Page size".into(),
            value: Some(label.into()),
            options: SIZES
                .iter()
                .map(|(value, label, ..)| SelectOption {
                    value: (*value).into(),
                    label: (*label).into(),
                })
                .collect(),
            ..button(VIEWPORT, "")
        },
    ]
}

/// The toolbar as outline lines, before the page's own.
pub(crate) fn outline(controls: &[Element]) -> String {
    let mut lines = vec!["- toolbar".to_string()];
    for control in controls {
        lines.push(format!(
            "  - {} [ref={}]",
            control.describe(),
            control.reference
        ));
    }
    lines.join("\n")
}

/// The width and height a toolbar value names.
pub(crate) fn size(value: &str) -> Option<(f64, f64)> {
    SIZES
        .iter()
        .find(|(known, ..)| *known == value)
        .map(|(.., width, height)| (*width, *height))
}
