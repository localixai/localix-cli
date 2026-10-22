//! Without a chat model, the decision model still needs strings to type. Code finds candidates in
//! the goal and the decision model picks one (TypeSafe's "pre-parsed value extraction"): what it
//! types is always a verbatim span of the goal.

const MAX_CANDIDATES: usize = 40;
const STOP: &[&str] = &[
    "a", "an", "the", "and", "or", "to", "in", "on", "at", "for", "of", "from", "with", "by",
    "find", "search", "open", "go", "show", "get", "me", "my", "is", "are", "then", "it", "that",
    "this", "and", "please", "page", "site", "click", "type", "enter", "select", "choose", "look",
    "up",
];

/// Quoted strings first, then word runs of one to four words that do not start or end on a
/// function word, longest first.
pub fn candidates(goal: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |value: &str| {
        let value = value
            .trim()
            .trim_matches(|c: char| ",.;:!?()".contains(c))
            .trim();
        if !value.is_empty()
            && !out.iter().any(|known| known == value)
            && out.len() < MAX_CANDIDATES
        {
            out.push(value.to_string());
        }
    };
    for quote in ['"', '\'', '«', '“'] {
        let close = match quote {
            '«' => '»',
            '“' => '”',
            other => other,
        };
        let mut rest = goal;
        while let Some(start) = rest.find(quote) {
            let after = &rest[start + quote.len_utf8()..];
            let Some(end) = after.find(close) else { break };
            push(&after[..end]);
            rest = &after[end + close.len_utf8()..];
        }
    }
    let words: Vec<&str> = goal
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| ",.;:!?()\"'«»“”".contains(c)))
        .collect();
    let content = |word: &str| !word.is_empty() && !STOP.contains(&word.to_lowercase().as_str());
    for length in (1..=4).rev() {
        for window in words.windows(length) {
            if content(window[0]) && content(window[length - 1]) {
                push(&window.join(" "));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::candidates;

    #[test]
    fn spans_of_the_goal_are_the_only_candidates() {
        let found = candidates("Find flights from Zurich to London on \"20 Sep 2026\"");
        assert_eq!(found[0], "20 Sep 2026");
        assert!(found.contains(&"Zurich".to_string()));
        assert!(found.contains(&"London".to_string()));
        assert!(!found.iter().any(|value| value == "to" || value == "from"));
        assert!(found.len() <= 40);
    }
}
