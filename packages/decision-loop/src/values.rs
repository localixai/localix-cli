//! The decision model types only strings it is offered. Code finds candidates in
//! the goal and the decision model picks one: what it
//! types is always a verbatim span of the goal.

const MAX_CANDIDATES: usize = 40;
/// A sentence longer than this is a list of steps, not something to type.
const MAX_SENTENCE: usize = 12;

/// The quoted strings, when the goal has any: whoever quoted them said what to type. Otherwise
/// each sentence of the goal as written (a search query is the whole request, not four words of
/// it), then every run of one to four words, longest first. No word lists: the model, not code,
/// tells a name from a function word, in any language.
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
    let trim = |w: &str| {
        w.trim_matches(|c: char| ",.;:!?()\"'«»“”".contains(c))
            .to_string()
    };
    for sentence in sentences(goal) {
        let words: Vec<String> = sentence.split_whitespace().map(trim).collect();
        if words.len() > 4 && words.len() <= MAX_SENTENCE {
            push(&mut out, trim(sentence.trim()).as_str());
        }
    }
    let words: Vec<String> = goal
        .split_whitespace()
        .map(trim)
        .filter(|w| !w.is_empty())
        .collect();
    for length in (1..=4).rev() {
        for window in words.windows(length) {
            push(&mut out, &window.join(" "));
        }
    }
    out
}

/// The goal cut where a sentence or a clause ends: a stop, a colon or a semicolon followed by a
/// space, or a line break. An address ("x.org/a.b") and a time ("19:30") stay whole.
fn sentences(goal: &str) -> Vec<&str> {
    let mut out = vec![];
    let mut start = 0;
    let mut chars = goal.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let next = chars.peek().map(|&(_, n)| n);
        let ends = c == '\n' || (".;:!?".contains(c) && next.is_none_or(char::is_whitespace));
        if ends {
            out.push(&goal[start..i]);
            start = i + c.len_utf8();
        }
    }
    out.push(&goal[start..]);
    out.into_iter().filter(|s| !s.trim().is_empty()).collect()
}

#[cfg(test)]
#[path = "values_tests.rs"]
mod tests;
