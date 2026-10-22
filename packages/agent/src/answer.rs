//! The agent's final reply: the answer for the person, and its closing `RESULT:` line saying
//! whether the goal was reached.

/// The answer without its closing `RESULT:` line, and what that line says.
pub fn read(text: &str) -> (String, Option<bool>) {
    let text = text.trim();
    let Some(at) = text.rfind("RESULT:") else {
        return (text.to_string(), None);
    };
    let verdict = text[at + "RESULT:".len()..].trim().to_lowercase();
    let achieved = if verdict.starts_with("not") {
        Some(false)
    } else if verdict.starts_with("achieved") {
        Some(true)
    } else {
        None
    };
    (text[..at].trim().to_string(), achieved)
}

#[cfg(test)]
#[path = "answer_tests.rs"]
mod tests;
