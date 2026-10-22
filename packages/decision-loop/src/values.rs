//! The decision model types only strings it is offered. Code finds candidates in
//! the goal and the decision model picks one: what it
//! types is always a verbatim span of the goal.

const MAX_CANDIDATES: usize = 40;
const STOP: &[&str] = &[
    "a", "an", "the", "and", "or", "to", "in", "on", "at", "for", "of", "from", "with", "by",
    "find", "search", "open", "go", "show", "get", "me", "my", "is", "are", "then", "it", "that",
    "this", "and", "please", "view", "site", "click", "type", "enter", "select", "choose", "look",
    "up",
];

/// The quoted strings, when the goal has any: whoever quoted them said what to type. Otherwise
/// word runs of one to four words that do not start or end on a function word, longest first.
pub fn candidates(goal: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let push = |out: &mut Vec<String>, value: &str| {
        if !value.is_empty()
            && !out.iter().any(|known| known == value)
            && out.len() < MAX_CANDIDATES
        {
            out.push(value.to_string());
        }
    };
    // A quoted string is taken as written ("SuperSecretPassword!"). An apostrophe quotes only
    // between word boundaries, so "don't" and "it's" quote nothing.
    for (open, close) in [('"', '"'), ('\'', '\''), ('«', '»'), ('“', '”')] {
        let mut at = 0;
        while let Some(start) = goal[at..].find(open).map(|i| at + i) {
            let inner = start + open.len_utf8();
            let Some(end) = goal[inner..].find(close).map(|i| inner + i) else {
                break;
            };
            let after = end + close.len_utf8();
            let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
            let bounded = open != '\''
                || (!word(goal[..start].chars().next_back())
                    && !word(goal[after..].chars().next()));
            if bounded {
                push(&mut out, goal[inner..end].trim());
                at = after;
            } else {
                at = inner;
            }
        }
    }
    if !out.is_empty() {
        return out;
    }
    let words: Vec<&str> = goal
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| ",.;:!?()\"'«»“”".contains(c)))
        .collect();
    let content = |word: &str| !word.is_empty() && !STOP.contains(&word.to_lowercase().as_str());
    for length in (1..=4).rev() {
        for window in words.windows(length) {
            if content(window[0]) && content(window[length - 1]) {
                push(&mut out, &window.join(" "));
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
        let found = candidates("Find flights from Zurich to London on 20 Sep 2026");
        assert!(found.contains(&"20 Sep 2026".to_string()));
        let password = candidates("Log in as \"tomsmith\" with password \"SuperSecretPassword!\"");
        assert_eq!(password, ["tomsmith", "SuperSecretPassword!"]);
        assert_eq!(candidates("Add 'buy milk' to the list"), ["buy milk"]);
        assert!(candidates("Don't stop, it's Paris").contains(&"Paris".to_string()));
        assert!(found.contains(&"Zurich".to_string()));
        assert!(found.contains(&"London".to_string()));
        assert!(!found.iter().any(|value| value == "to" || value == "from"));
        assert!(found.len() <= 40);
    }
}
