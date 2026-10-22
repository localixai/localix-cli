use super::*;

#[test]
fn each_provider_is_named_by_its_id_and_offers_its_default_model() {
    for provider in Provider::ALL {
        assert_eq!(Provider::named(provider.id()), Some(provider));
        let models = provider.models();
        assert!(
            models.iter().any(|m| m.id == provider.default_model()),
            "{provider:?}"
        );
        assert!(!provider.methods().is_empty());
    }
    assert_eq!(Provider::named("nobody"), None);
}
