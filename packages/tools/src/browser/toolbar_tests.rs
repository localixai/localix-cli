use super::*;

#[test]
fn a_browsers_reload_keys_are_its_reload_and_other_keys_are_the_pages() {
    for key in [
        "Ctrl+R",
        "ctrl+r",
        "Meta+R",
        "Cmd+Shift+R",
        "Control+r",
        "F5",
        "Shift+F5",
    ] {
        assert_eq!(shortcut(key), Some(RELOAD), "{key}");
    }
    for key in [
        "r",
        "R",
        "Shift+R",
        "Alt+R",
        "Ctrl+Alt+R",
        "F4",
        "Ctrl++",
        "Enter",
    ] {
        assert_eq!(shortcut(key), None, "{key}");
    }
}

#[test]
fn the_toolbar_offers_back_and_reload_and_no_page_size_to_pick_by_mistake() {
    let named: Vec<String> = controls().iter().map(|c| c.describe()).collect();
    assert_eq!(named, ["button \"Back\"", "button \"Reload\""]);
}
