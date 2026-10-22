use super::*;

#[test]
fn a_picture_keeps_its_shape_within_the_long_edge() {
    assert_eq!(fitted(2560.0, 1600.0), (1280.0, 800.0));
    assert_eq!(fitted(780.0, 1688.0), (591.0, 1280.0));
    assert_eq!(
        fitted(400.0, 200.0),
        (400.0, 200.0),
        "a small part is not enlarged"
    );
}
