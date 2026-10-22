use super::candidates;

#[test]
fn spans_of_the_goal_are_the_only_candidates() {
    let found = candidates("Find flights from Zurich to London on 20 Sep 2026");
    assert!(found.contains(&"20 Sep 2026".to_string()));
    let password = candidates("Log in as \"tomsmith\" with password \"SuperSecretPassword!\"");
    assert_eq!(password, ["tomsmith", "SuperSecretPassword!"]);
    assert_eq!(candidates("Add 'buy milk' to the list"), ["buy milk"]);
    assert!(candidates("Don't stop, it's Paris").contains(&"Paris".to_string()));
    assert!(found.contains(&"Zurich".to_string()));
    assert!(found.contains(&"London".to_string()));
    assert!(found.len() <= 40);
}

#[test]
fn a_sentence_is_offered_whole_in_any_language() {
    let goal = "encontrar los billetes más baratos de Chipre a Georgia en noviembre";
    assert_eq!(candidates(goal)[0], goal);
    let steps =
        candidates("Open https://x.org/a.b and: order at 19:30. Then pay with the saved card now");
    assert!(
        steps.contains(&"Then pay with the saved card now".to_string()),
        "{steps:?}"
    );
    assert!(
        !steps.iter().any(|s| s.contains("19:30. Then")),
        "{steps:?}"
    );
}
