use super::*;

#[test]
fn each_model_starts_without_reasoning_where_it_can() {
    let models = models();
    assert!(models.iter().any(|model| model.id == DEFAULT_MODEL));
    for model in &models {
        assert!(model.efforts.is_sorted(), "{}", model.id);
        assert_eq!(model.default, model.efforts[0], "{}", model.id);
    }
    assert_eq!(entry(DEFAULT_MODEL).default, Off);
    assert_eq!(entry("anthropic/claude-opus-5.5").default, Low);
}

#[test]
fn a_model_off_the_list_is_offered_no_level() {
    let model = entry("vendor/new-model");
    assert_eq!(model.offered(), [Off]);
}
