//! `doctor`: what it checks, and its exit code when a sign-in is missing.

use crate::common::home::{Home, text};
use crate::common::model::Model;

#[test]
fn doctor_without_a_key_checks_the_rest_then_exits_three() {
    let home = Home::new();
    let out = home.run(&["doctor"]);
    assert_eq!(out.status.code(), Some(3), "{}", text(&out.stderr));
    let shown = text(&out.stdout);
    assert!(shown.contains("shell: "), "{shown}");
    assert!(shown.contains("browser: "), "{shown}");
    assert!(shown.contains("OpenRouter: not signed in"), "{shown}");
    assert!(shown.contains("decision model: "), "{shown}");
}

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "a test runs the binary to its end, against a scripted model"
)]
fn a_decision_model_that_answers_nothing_fails_the_check() {
    let home = Home::new();
    // Every request gets a chat completion: right for the agent model, no answers for the other.
    let model = Model::start(vec![]);
    let out = home.with_model(&model, &["doctor"]).output().unwrap();
    let shown = text(&out.stdout);
    assert!(shown.contains("✓ agent model "), "{shown}");
    assert!(
        shown.contains("✗ decision model ") && shown.contains("no answer"),
        "{shown}"
    );
    assert_eq!(out.status.code(), Some(3), "{shown}");
}
