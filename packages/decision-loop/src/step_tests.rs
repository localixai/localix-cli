use super::*;

#[test]
fn an_action_that_changed_only_what_is_out_of_sight_acted_but_the_view_looks_the_same() {
    let before = Observation {
        location: "http://x/shop".into(),
        text: "Maidenhair fern added to cart".into(),
        revision: 4,
        ..Observation::default()
    };
    // A second "Add": the same words on screen, a counter above it out of sight went up.
    let after = Observation {
        revision: 6,
        ..before.clone()
    };
    assert!(acted(&before, &after));
    // A wait for what the model can see is not ended by it (a ticker out of sight).
    assert!(!changed(&before, &after));
    assert!(!acted(&before, &before.clone()));
}
