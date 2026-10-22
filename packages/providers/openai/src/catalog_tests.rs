use super::*;

#[test]
fn every_model_starts_at_a_level_it_offers() {
    let models = models();
    assert!(models.iter().any(|model| model.id == DEFAULT_MODEL));
    for model in models {
        assert!(model.efforts.contains(&model.default), "{}", model.id);
        assert!(
            model.efforts.is_sorted(),
            "{} lists its levels lowest first",
            model.id
        );
    }
}

#[test]
fn an_unknown_model_is_offered_no_level() {
    let model = entry("gpt-next");
    assert_eq!(
        (model.name.as_str(), model.offered()),
        ("gpt-next", &[Effort::Off][..])
    );
    assert_eq!(entry("gpt-5.5").efforts.last(), Some(&XHigh));
}
