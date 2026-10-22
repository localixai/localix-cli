use std::cell::RefCell;
use std::collections::VecDeque;

use contracts::catalog::Effort;

use super::*;
use crate::picker::{Lines, Picker};

/// A person who picks from a script, and remembers what each step offered.
#[derive(Default)]
struct Scripted {
    picks: VecDeque<Option<usize>>,
    shown: Vec<(String, Vec<Item>)>,
}

impl Scripted {
    fn new(picks: &[Option<usize>]) -> Self {
        Self {
            picks: picks.iter().copied().collect(),
            shown: vec![],
        }
    }

    fn titles(&self) -> Vec<&str> {
        self.shown.iter().map(|(title, _)| title.as_str()).collect()
    }

    fn offered(&self, title: &str) -> Vec<(String, bool)> {
        let (_, items) = self
            .shown
            .iter()
            .find(|(shown, _)| shown.starts_with(title))
            .unwrap();
        items
            .iter()
            .map(|item| (item.label.clone(), item.marked))
            .collect()
    }
}

impl Picker for Scripted {
    fn pick(&mut self, title: &str, items: &[Item]) -> Option<usize> {
        self.shown.push((title.to_string(), items.to_vec()));
        self.picks.pop_front().expect("a pick was scripted")
    }
}

impl Lines for Scripted {
    fn line(&mut self, _: &str, _: bool) -> Option<String> {
        None
    }
}

/// What the steps chose, and each sign-in they asked for.
type Outcome = (Result<Option<Choice>, String>, Vec<(Provider, Method)>);

/// Runs the steps with OpenAI signed in or not.
fn steps(current: &Choice, openai_signed_in: bool, terminal: &mut Scripted) -> Outcome {
    let asked = RefCell::new(vec![]);
    let signed_in = |provider| {
        (provider == Provider::OpenAi && openai_signed_in).then(|| "ChatGPT account a".to_string())
    };
    let sign_in = |provider, method, _: &mut dyn Terminal| {
        asked.borrow_mut().push((provider, method));
        Ok("Signed in.".to_string())
    };
    let world = World {
        signed_in: &signed_in,
        sign_in: &sign_in,
        say: &|_| {},
    };
    let chosen = choose(current, terminal, &world);
    (chosen, asked.into_inner())
}

fn openrouter() -> Choice {
    Choice::new(Provider::OpenRouter, openrouter::catalog::DEFAULT_MODEL)
}

fn names(efforts: &[Effort]) -> Vec<String> {
    efforts
        .iter()
        .map(|effort| effort.name().to_string())
        .collect()
}

#[test]
fn signing_in_to_openai_then_choosing_a_model_and_a_level_it_takes() {
    // OpenAI, its default model (Enter), then the third level offered.
    let mut terminal = Scripted::new(&[Some(0), Some(0), Some(2)]);
    let (chosen, asked) = steps(&openrouter(), false, &mut terminal);
    let chosen = chosen.unwrap().unwrap();
    assert_eq!(asked, [(Provider::OpenAi, Method::Browser)]);
    assert_eq!(
        terminal.titles()[..2],
        ["Provider", "Model (OpenAI (sign in with ChatGPT))"]
    );
    assert_eq!(chosen.provider, Provider::OpenAi);
    assert_eq!(chosen.model(), openai::catalog::DEFAULT_MODEL);
    let offered = terminal.offered("Reasoning");
    let levels: Vec<String> = offered.iter().map(|(label, _)| label.clone()).collect();
    assert_eq!(
        levels,
        names(chosen.selection.model().offered()),
        "only the model's own levels"
    );
    assert_eq!(
        chosen.selection.effort(),
        chosen.selection.model().offered()[2]
    );
}

#[test]
fn another_model_starts_at_its_default_and_offers_only_its_levels() {
    let current = Choice::new(Provider::OpenAi, "gpt-6-sol");
    let current = Choice {
        selection: current.selection.with_effort(Effort::Ultra).unwrap(),
        ..current
    };
    let at = Provider::OpenAi
        .models()
        .iter()
        .position(|m| m.id == "gpt-5.5")
        .unwrap();
    // OpenAI (current), keep the sign-in, gpt-5.5, Enter on the marked level.
    let mut terminal = Scripted::new(&[Some(0), Some(0), Some(at), Some(1)]);
    let (chosen, asked) = steps(&current, true, &mut terminal);
    assert!(asked.is_empty(), "the sign-in was kept");
    let offered = terminal.offered("Reasoning");
    assert_eq!(
        offered,
        [
            ("low".into(), false),
            ("medium".into(), true),
            ("high".into(), false),
            ("xhigh".into(), false)
        ]
    );
    assert_eq!(chosen.unwrap().unwrap().selection.effort(), Effort::Medium);
}

#[test]
fn the_same_model_keeps_its_level_marked() {
    let current = Choice::new(Provider::OpenAi, "gpt-6-sol");
    let current = Choice {
        selection: current.selection.with_effort(Effort::High).unwrap(),
        ..current
    };
    let at = Provider::OpenAi
        .models()
        .iter()
        .position(|m| m.id == "gpt-6-sol")
        .unwrap();
    let mut terminal = Scripted::new(&[Some(0), Some(0), Some(at), Some(2)]);
    let (chosen, _) = steps(&current, true, &mut terminal);
    let marked: Vec<String> = terminal
        .offered("Reasoning")
        .into_iter()
        .filter(|(_, marked)| *marked)
        .map(|(label, _)| label)
        .collect();
    assert_eq!(marked, ["high"]);
    assert_eq!(chosen.unwrap().unwrap().selection.effort(), Effort::High);
}

#[test]
fn openrouter_asks_how_to_sign_in_and_a_model_with_one_level_skips_the_step() {
    let current = Choice::new(Provider::OpenRouter, "vendor/own-model");
    // OpenRouter, paste a key, the current (custom) model listed first.
    let mut terminal = Scripted::new(&[Some(1), Some(0), Some(0)]);
    let (chosen, asked) = steps(&current, false, &mut terminal);
    assert_eq!(asked, [(Provider::OpenRouter, Method::Key)]);
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
