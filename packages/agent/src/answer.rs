//! The agent's final reply: the answer for the person, and its closing `RESULT:` line saying
//! whether the goal was reached.

/// The answer without its closing `RESULT:` line, and what that line says.
pub fn read(text: &str) -> (String, Option<bool>) {
    let text = text.trim();
    let Some((answer, verdict)) = text.rsplit_once("RESULT:") else {
        return (text.to_string(), None);
    };
    let verdict = verdict.trim().to_lowercase();
    let achieved = if verdict.starts_with("not") {
        Some(false)
    } else if verdict.starts_with("achieved") {
        Some(true)
    } else {
        None
    };
    (answer.trim().to_string(), achieved)
}

#[cfg(test)]
#[path = "answer_tests.rs"]
mod tests;
