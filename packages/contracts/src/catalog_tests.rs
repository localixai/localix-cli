use super::*;

fn entry(efforts: &[Effort], default: Effort) -> ModelEntry {
    ModelEntry {
        id: "m".into(),
        name: "M".into(),
        efforts: efforts.to_vec(),
        default,
    }
}

#[test]
fn a_selection_starts_at_the_models_default() {
    let model = entry(&[Effort::Low, Effort::Medium, Effort::High], Effort::Medium);
    assert_eq!(Selection::new(model).effort(), Effort::Medium);
}

#[test]
fn a_default_the_model_does_not_offer_becomes_its_lowest_level() {
    let model = entry(&[Effort::Low, Effort::High], Effort::Ultra);
    assert_eq!(Selection::new(model).effort(), Effort::Low);
}

#[test]
fn only_an_offered_level_can_be_chosen() {
    let selection = Selection::new(entry(&[Effort::Low, Effort::High], Effort::Low));
    assert_eq!(
        selection.with_effort(Effort::High).map(|s| s.effort()),
        Some(Effort::High)
    );
    assert_eq!(selection.with_effort(Effort::Max), None);
    assert_eq!(selection.with_effort(Effort::Off), None);
}

#[test]
fn a_model_with_no_levels_runs_with_none() {
    let selection = Selection::new(entry(&[], Effort::High));
    assert_eq!(selection.effort(), Effort::Off);
    assert_eq!(selection.model().offered(), [Effort::Off]);
    assert!(selection.with_effort(Effort::Low).is_none());
}

#[test]
fn levels_read_by_name_and_none_is_off() {
    assert_eq!(Effort::named(" XHigh "), Some(Effort::XHigh));
    assert_eq!(Effort::named("none"), Some(Effort::Off));
    assert_eq!(Effort::named("turbo"), None);
    for effort in Effort::ALL {
        assert_eq!(Effort::named(effort.name()), Some(effort));
    }
}
