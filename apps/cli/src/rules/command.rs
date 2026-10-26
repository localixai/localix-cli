//! A command line as rules read it: split into the commands it runs, wrappers looked through.
//! An allow rule needs every part to be one it can read for sure; a deny rule looks at every part
//! it can find, inside substitutions too.

/// The commands of `line` an allow rule may judge, or `None` when it cannot be read for sure: a
/// substitution, a here-document, a write to a file, an unclosed quote, a dangling operator, or a
/// command that runs another (`eval`, `sh -c`).
pub(super) fn sure(line: &str) -> Option<Vec<String>> {
    let split = split(line);
    if split.unsure || split.parts.iter().any(String::is_empty) {
        return None;
    }
    let parts: Option<Vec<String>> = split.parts.iter().map(|part| plain(part)).collect();
    parts.filter(|parts| parts.iter().all(|part| !runs_another(part)))
}

/// Every command of `line` a deny rule looks at, past any assignment and wrapper.
pub(super) fn every(line: &str) -> Vec<String> {
    let split = split(line);
    let parts = split.parts.iter().filter(|part| !part.is_empty());
    parts.map(|part| unwrapped(part).0).collect()
}

/// `text` matched by `pattern`, where `*` stands for any text, spaces too. A pattern whose one
/// `*` ends it after a space also matches the bare command: `ls *` matches `ls`.
pub(super) fn matches(pattern: &str, text: &str) -> bool {
    let pattern = pattern
        .strip_suffix(":*")
        .map_or(pattern.to_string(), |head| format!("{head} *"));
    if let Some(head) = pattern.strip_suffix(" *")
        && !head.contains('*')
        && text == head
    {
        return true;
    }
    glob(pattern.as_bytes(), text.as_bytes())
}

fn glob(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((b'*', rest)) => (0..=text.len()).any(|at| glob(rest, &text[at..])),
        Some((first, rest)) => text
            .split_first()
            .is_some_and(|(c, tail)| c == first && glob(rest, tail)),
    }
}

/// A part with no assignment before it, its wrappers looked through; `None` when an assignment
/// leads it (an allow rule never reaches past one: `LD_PRELOAD=x npm test`).
fn plain(part: &str) -> Option<String> {
    let (command, assigned) = unwrapped(part);
    (!assigned).then_some(command)
}

/// Whether a part hands text to something that runs it, or writes a file through `tee`, or
/// `find` runs or deletes, or `git` is told a program to run: what the rule names is not what runs.
fn runs_another(part: &str) -> bool {
    let words: Vec<&str> = part.split_whitespace().collect();
    let has = |flag: &str| words.iter().skip(1).any(|word| *word == flag);
    match words.first().copied().unwrap_or_default() {
        "eval" | "exec" | "source" | "." => true,
        "sh" | "bash" | "zsh" | "dash" | "fish" => has("-c"),
        "tee" => words
            .iter()
            .skip(1)
            .any(|word| !word.starts_with('-') && *word != "/dev/null"),
        "find" => [
            "-exec", "-execdir", "-ok", "-okdir", "-delete", "-fprint", "-fprintf", "-fls",
        ]
        .iter()
        .any(|flag| has(flag)),
        program if program.rsplit('/').next() == Some("git") => super::git::runs_another(part),
        _ => false,
    }
}

/// `part` past its leading assignments and wrappers, and whether it had an assignment.
fn unwrapped(part: &str) -> (String, bool) {
    let mut rest = part.trim();
    let mut assigned = false;
    loop {
        let (word, after) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let after = after.trim_start();
        let skip = match word {
            _ if is_assignment(word) => {
                assigned = true;
                0
            }
            "time" | "nohup" | "builtin" | "noglob" => 0,
            "command" if !after.starts_with("-v") && !after.starts_with("-V") => 0,
            "xargs" if !after.starts_with('-') => 0,
            "timeout" | "nice" | "stdbuf" => flags(word, after),
            _ => return (rest.to_string(), assigned),
        };
        rest = after;
        for _ in 0..skip {
            rest = rest
                .split_once(char::is_whitespace)
                .map_or("", |(_, tail)| tail.trim_start());
        }
    }
}

/// How many words of a wrapper's own come before the command it runs.
fn flags(wrapper: &str, after: &str) -> usize {
    let words: Vec<&str> = after.split_whitespace().collect();
    let options = words
        .iter()
        .take_while(|word| word.starts_with('-'))
        .count();
    let valued = |word: &&&str| {
        [
            "-n",
            "-s",
            "-k",
            "-i",
            "-o",
            "-e",
            "--signal",
            "--kill-after",
        ]
        .contains(*word)
    };
    let values = words.iter().take(options).filter(valued).count();
    let options = options + values;
    match wrapper {
        "timeout" => options + 1,
        _ => options,
    }
}

fn is_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// A line cut at its operators, outside quotes, with a mark for what an allow cannot be sure of.
struct Split {
    parts: Vec<String>,
    unsure: bool,
}

fn split(line: &str) -> Split {
    let chars: Vec<char> = line.chars().collect();
    let mut split = Split {
        parts: vec![],
        unsure: false,
    };
    let mut part = String::new();
    let (mut single, mut double) = (false, false);
    let mut at = 0;
    while let Some(&c) = chars.get(at) {
        let next = chars.get(at + 1).copied();
        let cut = |width: usize, split: &mut Split, part: &mut String| {
            split.parts.push(part.trim().to_string());
            part.clear();
            width
        };
        let width = match c {
            '\'' if !double => {
                single = !single;
                part.push(c);
                1
            }
            _ if single => {
                part.push(c);
                1
            }
            '\\' => {
                part.push(c);
                part.extend(next);
                2
            }
            '"' => {
                double = !double;
                part.push(c);
                1
            }
            '`' => {
                split.unsure = true;
                cut(1, &mut split, &mut part)
            }
            '$' if next == Some('(') => {
                split.unsure = true;
                cut(2, &mut split, &mut part)
            }
            ')' if !double => cut(1, &mut split, &mut part),
            _ if double => {
                part.push(c);
                1
            }
            '<' | '>' if next == Some('(') => {
                split.unsure = true;
                cut(2, &mut split, &mut part)
            }
            '(' => {
                split.unsure = true;
                cut(1, &mut split, &mut part)
            }
            '<' if next == Some('<') => {
                split.unsure = true;
                part.push(c);
                1
            }
            '>' => redirect(&chars, at, &mut part, &mut split),
            '&' if next == Some('>') => {
                split.unsure = true;
                part.push(c);
                1
            }
            '&' | '|' if next == Some(c) => cut(2, &mut split, &mut part),
            '|' if next == Some('&') => cut(2, &mut split, &mut part),
            '&' | '|' | ';' | '\n' => cut(1, &mut split, &mut part),
            _ => {
                part.push(c);
                1
            }
        };
        at += width;
    }
    split.unsure |= single || double;
    split.parts.push(part.trim().to_string());
    split
}

/// An output redirect kept in the part; one to a file, not `/dev/null` or a descriptor, makes the
/// line unsure (the rule names the command, not what it writes).
fn redirect(chars: &[char], at: usize, part: &mut String, split: &mut Split) -> usize {
    let mut end = at + 1;
    if chars.get(end) == Some(&'>') {
        end += 1;
    }
    let rest: String = chars[end..].iter().collect();
    let rest = rest.trim_start();
    let target = rest
        .split(|c: char| c.is_whitespace() || ";|&".contains(c))
        .next()
        .unwrap_or_default();
    let descriptor = rest
        .strip_prefix('&')
        .is_some_and(|fd| fd.starts_with(|c: char| c.is_ascii_digit() || c == '-'));
    if !descriptor && target != "/dev/null" {
        split.unsure = true;
    }
    if descriptor {
        let fd: String = rest
            .chars()
            .skip(1)
            .take_while(|c| c.is_ascii_digit() || *c == '-')
            .collect();
        part.extend(&chars[at..end]);
        part.push('&');
        part.push_str(&fd);
        return end - at
            + (chars[end..]
                .iter()
                .take_while(|c| c.is_whitespace())
                .count())
            + 1
            + fd.len();
    }
    part.extend(&chars[at..end]);
    end - at
}
