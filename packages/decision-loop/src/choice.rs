//! Reading a decision model's choice: only an option that was offered, from a sound distribution,
//! ever executes.

use agent::model::Answer;
use serde_json::{Map, Value};

/// The choice and its probability when the answer is one of `offered` and its distribution is
/// sound: finite, summing to one, with the choice on top. Anything else executes nothing.
pub fn valid_choice<'a>(
    answer: Option<&'a Answer>,
    offered: &Map<String, Value>,
) -> Option<(&'a str, f64)> {
    let Answer::Choice {
        choice,
        probabilities,
        ..
    } = answer?
    else {
        return None;
    };
    if !offered.contains_key(choice) {
        return None;
    }
    let sound = probabilities
        .values()
        .all(|p| p.is_finite() && (0.0..=1.0).contains(p))
        && (probabilities.values().sum::<f64>() - 1.0).abs() < 0.02
        && probabilities.keys().all(|key| offered.contains_key(key));
    let top = probabilities.values().cloned().fold(0.0, f64::max);
    let mine = *probabilities.get(choice)?;
    (sound && mine >= top - 1e-6).then_some((choice.as_str(), mine))
}

#[cfg(test)]
mod tests {
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
}
