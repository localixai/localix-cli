//! The slash commands at the prompt.

use super::LONG;
use crate::common::model::{Model, Reply};
use crate::common::term::{Term, key};

#[test]
fn slash_down_enter_opens_model_which_applies_the_one_chosen() {
    // The bug a person hit: "/" alone, Down to a command, Enter said "unknown command /".
    // `/model` is the second command offered: Down marks it, Enter opens its picker.
    let model = Model::start(vec![Reply::text("Hi.\nRESULT: achieved")]);
    let mut term = Term::start(&model, &[]);
    term.press("/");
    term.wait_for("/model", LONG);
    term.press(key::DOWN).press(key::ENTER);
    let screen = term.wait_for("• Model", LONG);
    assert!(!screen.contains("unknown command"), "{screen}");
    assert!(screen.contains("OpenRouter"), "{screen}");
    assert!(screen.contains("❯"), "a model is marked:\n{screen}");
    // Off the current model, so a /model that applied nothing would show.
    term.press(key::DOWN).press(key::ENTER);
    // One Enter confirms both the model and its inline reasoning level.
    term.wait_for("Now openrouter", LONG);
    term.type_text("hello").press(key::ENTER);
    term.wait_for("Worked for", LONG);
    let used = model.requests()[0]["model"].as_str().unwrap().to_string();
    assert_ne!(used, "anthropic/claude-haiku-5.5");
    let settings = std::fs::read_to_string(term.file("../config/llx/settings.json")).unwrap();
    assert!(settings.contains(&used), "{used} is not in {settings}");
}

#[test]
fn the_header_names_the_model_model_switched_to() {
    // The bug a person saw: after /model the header line still named the model the session
    // started with. A switch prints the header again, with the model now in use.
    let model = Model::start(vec![]);
    let mut term = Term::start(&model, &[]);
    term.type_text("/model").press(key::ENTER);
    term.wait_for("• Model", LONG);
    term.press(key::DOWN).press(key::ENTER);
    let screen = term.wait_for("Now openrouter · ", LONG);
    let chosen = screen
        .lines()
        .find_map(|line| line.trim().strip_prefix("Now openrouter · "))
        .and_then(|rest| rest.split(' ').next())
        .unwrap()
        .to_string();
    term.wait_for(&format!("· openrouter · {chosen} ·"), LONG);
}

#[test]
fn slash_mode_offers_the_modes_the_server_says() {
    let model = Model::start(vec![]);
    let mut term = Term::start(&model, &[]);
    term.type_text("/mode by");
    let screen = term.wait_for("bypass", LONG);
    assert!(
        screen.contains("Everything runs without asking"),
        "{screen}"
    );
}
