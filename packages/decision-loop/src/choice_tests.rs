use serde_json::json;

use super::*;

fn offered(keys: &[&str]) -> Map<String, Value> {
    keys.iter()
        .map(|key| (key.to_string(), json!(key)))
        .collect()
}

fn choice(choice: &str, probabilities: &[(&str, f64)]) -> Answer {
    Answer::Choice {
        choice: choice.into(),
        probabilities: probabilities
            .iter()
            .map(|(k, p)| (k.to_string(), *p))
            .collect(),
        confidence: 1.0,
    }
}

#[test]
fn accepts_only_a_sound_choice_among_the_offered_keys() {
    let keys = offered(&["CLICK", "DONE"]);
    let good = choice("CLICK", &[("CLICK", 0.9), ("DONE", 0.1)]);
    assert_eq!(valid_choice(Some(&good), &keys), Some(("CLICK", 0.9)));
    let unknown = choice("TYPE", &[("TYPE", 1.0)]);
    assert_eq!(valid_choice(Some(&unknown), &keys), None);
    let not_top = choice("DONE", &[("CLICK", 0.9), ("DONE", 0.1)]);
    assert_eq!(valid_choice(Some(&not_top), &keys), None);
    let unsound = choice("CLICK", &[("CLICK", 0.9), ("DONE", 0.5)]);
    assert_eq!(valid_choice(Some(&unsound), &keys), None);
    assert_eq!(valid_choice(Some(&Answer::Noul(0.9)), &keys), None);
}
