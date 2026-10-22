use super::*;

#[test]
fn every_guide_has_a_description_and_a_body() {
    for (name, text) in SKILLS {
        assert!(!description(text).is_empty(), "{name} has no description");
        assert!(
            get(name).unwrap().starts_with("# "),
            "{name} body starts with its title"
        );
    }
    assert!(list().contains("testing"));
    assert_eq!(get("nope"), None);
}
