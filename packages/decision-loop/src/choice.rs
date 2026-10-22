//! Reading a decision model's choice: only an option that was offered, from a sound distribution,
//! ever executes.

use contracts::model::Answer;
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
#[path = "choice_tests.rs"]
mod tests;
