use super::*;

fn drag(said: &str) -> Option<Drag> {
    if let Cmd::MiniDrag(drag) = heard(said)? {
        return Some(drag);
    }
    None
}

#[test]
fn a_press_a_move_and_a_release_drag_the_mini() {
    assert_eq!(drag("press"), Some(Drag::Press));
    assert_eq!(drag("by 12 -4.5"), Some(Drag::By(12.0, -4.5)));
    assert_eq!(drag("let"), Some(Drag::Let));
}

#[test]
fn a_click_expands_and_anything_else_is_nothing() {
    assert!(matches!(
        heard("expand"),
        Some(Cmd::Presence(Event::Expand))
    ));
    for said in ["", "drag", "by", "by 1", "by x 2", "expand now"] {
        assert!(heard(said).is_none(), "{said}");
    }
}
