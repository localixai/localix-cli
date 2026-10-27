use super::*;

#[test]
fn its_button_expands_and_anything_else_is_nothing() {
    assert!(matches!(
        heard("expand"),
        Some(Cmd::Presence(Event::Expand))
    ));
    for said in ["", "drag", "press", "by 1 2", "expand now"] {
        assert!(heard(said).is_none(), "{said}");
    }
}
