use super::*;

#[test]
fn a_shown_window_is_an_ordinary_one_and_only_an_invisible_one_floats() {
    assert_eq!(
        level(false),
        NORMAL_WINDOW_LEVEL,
        "a shown window never covers other apps"
    );
    assert_eq!(level(true), FLOATING_WINDOW_LEVEL);
}
