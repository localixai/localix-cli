use std::sync::Mutex;

use contracts::catalog::Effort;

use super::*;
use crate::testing::Scripted;

/// What the steps chose, and each sign-in they asked for.
type Outcome = (Result<Option<Choice>, String>, Vec<(Provider, Method)>);

/// Runs the steps with OpenAI signed in or not.
fn steps(current: &Choice, openai_signed_in: bool, terminal: &mut Scripted) -> Outcome {
    let asked = Mutex::new(vec![]);
    let signed_in = |provider: &Provider| {
        (*provider == Provider::openai() && openai_signed_in)
            .then(|| "ChatGPT account a".to_string())
    };
    let sign_in = |provider, method, _: &mut dyn Terminal| {
        asked.lock().unwrap().push((provider, method));
        Ok("Signed in.".to_string())
    };
    let world = World {
        signed_in: &signed_in,
        sign_in: &sign_in,
        say: &|_| {},
    };
    let chosen = choose(current, terminal, &world);
    (chosen, asked.into_inner().unwrap())
}

fn openrouter() -> Choice {
    Choice::new(Provider::openrouter(), "anthropic/claude-haiku-5.5")
}

#[test]
fn signing_in_to_openai_then_choosing_a_model_and_a_level_it_takes() {
    // OpenAI, the ChatGPT sign-in, then its default model and level together.
    let models = Provider::openai().models();
    let luna = models.iter().position(|m| m.id == "gpt-6-luna").unwrap();
    let mut terminal = Scripted::new(&[Some(0), Some(0), Some(luna)]);
    let (chosen, asked) = steps(&openrouter(), false, &mut terminal);
    let chosen = chosen.unwrap().unwrap();
    assert_eq!(asked, [(Provider::openai(), Method::Browser)]);
    assert_eq!(
        terminal.titles()[..3],
        ["Provider", "Sign in with", "Model (OpenAI)"]
    );
    // How OpenAI is signed in to is said where the provider is picked, and nowhere else.
    let (_, providers) = &terminal.shown[0];
    let openai = providers
        .iter()
        .find(|item| item.label == "OpenAI")
        .unwrap();
    assert_eq!(
        openai.detail,
        "openai · not signed in · ChatGPT sign-in, or an API key"
    );
    assert_eq!(chosen.provider, Provider::openai());
    assert_eq!(chosen.model(), "gpt-6-luna");
    let marked = terminal
        .offered("Model")
        .into_iter()
        .position(|(_, marked)| marked);
    assert_eq!(marked, Some(luna), "the default is the one marked");
    assert_eq!(terminal.titles().len(), 3, "no separate reasoning menu");
    assert_eq!(chosen.selection.effort(), models[luna].default);
}

#[test]
fn the_model_list_keeps_the_current_level_or_starts_at_the_new_models_default() {
    for (effort, model) in [(Effort::Max, "gpt-5.5"), (Effort::High, "gpt-6-sol")] {
        let current = Choice::new(Provider::openai(), "gpt-6-sol");
        let current = Choice {
            selection: current.selection.with_effort(effort).unwrap(),
            ..current
        };
        let models = Provider::openai().models();
        let at = models.iter().position(|m| m.id == model).unwrap();
        // OpenAI (current), keep the sign-in, confirm the model and inline level.
        let mut terminal = Scripted::new(&[Some(0), Some(0), Some(at)]);
        let (chosen, asked) = steps(&current, true, &mut terminal);
        assert!(asked.is_empty(), "{model}: the sign-in was kept");
        let expected = if model == current.model() {
            effort
        } else {
            models[at].default
        };
        assert_eq!(terminal.titles().len(), 3);
        assert_eq!(
            chosen.unwrap().unwrap().selection.effort(),
            expected,
            "{model}"
        );
    }
}

#[test]
fn openrouter_asks_how_to_sign_in_and_a_model_with_one_level_skips_the_step() {
    let current = Choice::new(Provider::openrouter(), "vendor/own-model");
    // OpenRouter (fourth of the popular), paste a key, the current (custom) model listed first.
    let at = Provider::all()
        .iter()
        .position(|p| *p == Provider::openrouter())
        .unwrap();
    let mut terminal = Scripted::new(&[Some(at), Some(0), Some(0)]);
    let (chosen, asked) = steps(&current, false, &mut terminal);
    assert_eq!(asked, [(Provider::openrouter(), Method::Key)]);
    assert_eq!(
        terminal.offered("Sign in with")[0],
        ("Paste an API key".into(), true)
    );
    assert!(
        !terminal
            .titles()
            .iter()
            .any(|title| title.starts_with("Reasoning"))
    );
    let chosen = chosen.unwrap().unwrap();
    assert_eq!(
        (chosen.model(), chosen.selection.effort()),
        ("vendor/own-model", Effort::Off)
    );
}

#[test]
fn cancelling_any_step_changes_nothing() {
    for picks in [&[None][..], &[Some(0), None], &[Some(0), Some(0), None]] {
        let mut terminal = Scripted::new(picks);
        assert_eq!(
            steps(&openrouter(), true, &mut terminal).0,
            Ok(None),
            "{picks:?}"
        );
    }
}

#[test]
fn every_provider_is_offered_with_the_signed_in_marked_and_none_typed_by_id() {
    let mut terminal = Scripted::new(&[None]);
    let (chosen, _) = steps(&openrouter(), true, &mut terminal);
    assert_eq!(chosen, Ok(None));
    let (_, items) = &terminal.shown[0];
    assert_eq!(items.len(), Provider::all().len());
    assert!(!items.iter().any(|item| item.label.starts_with("Another")));
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels[..2], ["OpenAI ✓", "Anthropic"], "{labels:?}");
    // The id leads the detail: a search finds a provider by it.
    assert_eq!(items[0].detail, "openai · signed in: ChatGPT account a");
}

#[test]
fn the_popular_come_first_then_the_current_and_signed_in_then_the_rest_by_name() {
    let all: Vec<Provider> = ["zeta", "openai", "acme", "deepseek", "beta", "mid"]
        .iter()
        .map(|id| Provider(id.to_string()))
        .collect();
    let signed_in = |p: &Provider| (p.id() == "beta").then(|| "a key".to_string());
    let order: Vec<String> = offered(all, &Provider("mid".into()), &signed_in)
        .iter()
        .map(|p| p.id().to_string())
        .collect();
    assert_eq!(order, ["openai", "deepseek", "beta", "mid", "acme", "zeta"]);
}
