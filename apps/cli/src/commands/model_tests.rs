use contracts::catalog::{Effort, Selection};

use super::*;
use crate::testing::Scripted;

fn openrouter() -> Choice {
    Choice::new(Provider::openrouter(), "anthropic/claude-haiku-5.5")
}

#[test]
fn model_offers_the_models_of_the_providers_signed_in_to_then_a_level() {
    let current = openrouter();
    let openai_only =
        |provider: &Provider| (*provider == Provider::openai()).then(|| "a".to_string());
    let mut terminal = Scripted::new(&[None]);
    assert_eq!(pick_model(&current, &mut terminal, &openai_only), None);
    let offered = terminal.offered("Model");
    let marked: Vec<&String> = offered.iter().filter(|(_, m)| *m).map(|(l, _)| l).collect();
    assert_eq!(marked.len(), 1, "the current model is marked: {offered:?}");
    let openai_models = Provider::openai().models().len();
    let openrouter_models = Provider::openrouter().models().len();
    assert_eq!(
        offered.len(),
        openai_models + openrouter_models,
        "the current provider's models, and those of every provider signed in to"
    );
    // The first of OpenAI's models (OpenAI comes first), at its first level.
    let mut terminal = Scripted::new(&[Some(0), Some(0)]);
    let chosen = pick_model(&current, &mut terminal, &openai_only).unwrap();
    assert_eq!(chosen.provider, Provider::openai());
    assert_eq!(chosen.model(), Provider::openai().models()[0].id);
    assert_eq!(terminal.titles()[1].split(' ').next(), Some("Reasoning"));
    let nobody = |_: &Provider| None;
    let mut terminal = Scripted::new(&[None]);
    pick_model(&current, &mut terminal, &nobody);
    assert_eq!(terminal.offered("Model").len(), openrouter_models);
}

fn level(efforts: Vec<Effort>, at: Effort) -> Selection {
    let model = contracts::catalog::ModelEntry {
        id: "lab/model".into(),
        name: "Model".into(),
        default: at,
        efforts,
        context: 1000,
    };
    Selection::new(model).with_effort(at).unwrap()
}

#[test]
fn a_step_goes_a_level_up_or_down_and_says_when_there_is_none() {
    let levels = vec![Effort::Low, Effort::Medium, Effort::High];
    let up = stepped(&level(levels.clone(), Effort::Medium), true).unwrap();
    assert_eq!(up.effort(), Effort::High);
    let down = stepped(&level(levels.clone(), Effort::Medium), false).unwrap();
    assert_eq!(down.effort(), Effort::Low);
    assert_eq!(
        stepped(&level(levels, Effort::High), true),
        Err("lab/model takes no more: this is the most it reasons.\n".into())
    );
}

#[test]
fn a_model_with_no_levels_says_so_when_stepped() {
    for up in [true, false] {
        assert_eq!(
            stepped(&level(vec![], Effort::Off), up),
            Err("lab/model takes no reasoning levels.\n".into())
        );
    }
}
