use super::*;

fn options(provider: Provider, model: Option<&str>, reasoning: Option<&str>) -> ModelOptions {
    ModelOptions {
        provider: Some(provider),
        model: model.map(str::to_string),
        reasoning: reasoning.map(str::to_string),
        decision_model: "d".into(),
    }
}

fn at(choice: Choice, effort: Effort) -> Choice {
    let selection = choice.selection.with_effort(effort).unwrap();
    Choice {
        selection,
        ..choice
    }
}

#[test]
fn a_saved_choice_reads_back_and_a_level_its_model_lost_falls_to_the_default() {
    let choice = at(Choice::new(Provider::OpenAi, "gpt-6-sol"), Effort::Ultra);
    assert_eq!(Choice::from_json(&choice.to_json()), Some(choice));
    let stale = json!({"provider": "openai", "model": "gpt-5.5", "reasoning": "ultra"});
    let read = Choice::from_json(&stale).unwrap();
    assert_eq!(
        read.selection.effort(),
        Effort::Medium,
        "gpt-5.5 tops out at xhigh"
    );
    assert_eq!(
        Choice::from_json(&json!({"provider": "elsewhere", "model": "m"})),
        None
    );
}

#[test]
fn with_nothing_given_the_saved_choice_of_that_provider_is_used() {
    let saved = at(Choice::new(Provider::OpenAi, "gpt-6-sol"), Effort::High);
    let resolved = resolve(&options(Provider::OpenAi, None, None), Some(saved.clone()));
    assert_eq!(resolved, Ok(saved.clone()));
    let other = resolve(&options(Provider::OpenRouter, None, None), Some(saved)).unwrap();
    assert_eq!(other.model(), openrouter::catalog::DEFAULT_MODEL);
}

#[test]
fn another_model_starts_at_its_own_default() {
    let saved = at(Choice::new(Provider::OpenAi, "gpt-6-sol"), Effort::Ultra);
    let resolved = resolve(
        &options(Provider::OpenAi, Some("gpt-5.5"), None),
        Some(saved),
    )
    .unwrap();
    assert_eq!(
        (resolved.model(), resolved.selection.effort()),
        ("gpt-5.5", Effort::Medium)
    );
}

#[test]
fn a_level_the_model_does_not_take_is_a_usage_error() {
    let given = options(Provider::OpenAi, Some("gpt-5.5"), Some("xhigh"));
    assert_eq!(
        resolve(&given, None).unwrap().selection.effort(),
        Effort::XHigh
    );
    let refused = resolve(
        &options(Provider::OpenAi, Some("gpt-5.5"), Some("ultra")),
        None,
    );
    assert_eq!(
        refused,
        Err(Failure::Usage(
            "--reasoning ultra: gpt-5.5 takes low, medium, high, xhigh".into()
        ))
    );
    let unknown = resolve(
        &options(Provider::OpenRouter, Some("v/m"), Some("low")),
        None,
    );
    assert!(matches!(unknown, Err(Failure::Usage(m)) if m.ends_with("takes off")));
}
