//! A long list put to the person: typed letters search it.

use super::LONG;
use crate::common::model::{Model, Reply};
use crate::common::term::{Term, key};

#[test]
fn login_finds_a_provider_by_typing_and_enter_takes_it() {
    let model = Model::start(vec![Reply::text("Hi.\nRESULT: achieved")]);
    let mut term = Term::start(&model, &[]);
    term.type_text("/login").press(key::ENTER);
    term.wait_for("Search: ▏", LONG);
    term.type_text("deeps");
    let screen = term.wait_for("Search: deeps▏  1 match", LONG);
    assert!(screen.contains("❯ DeepSeek"), "{screen}");
    assert!(!screen.contains("OpenRouter"), "{screen}");
    term.press(key::ENTER);
    term.wait_for("Paste the DeepSeek key", LONG);
}
