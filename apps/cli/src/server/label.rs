//! How a call reads, in a few words: the verb while it runs and once done, and its subject. The
//! server knows the tools, so it says it for every call it reports (`protocol::Label`); the
//! interface and the trace only show it.

use serde_json::Value;

use crate::printable::one_line;

/// How the call of `name` with `args` reads.
pub(crate) fn label(name: &str, args: &Value) -> protocol::Label {
    let what = describe(name, args);
    protocol::Label {
        doing: what.verb.0.into(),
        done: what.verb.1.into(),
        subject: what.subject,
    }
}

/// `n` of `thing`, as `1 question` or `3 questions`.
fn count(n: usize, thing: &str) -> String {
    match n {
        1 => format!("1 {thing}"),
        n => format!("{n} {thing}s"),
    }
}

struct What {
    /// The verb while running and once done: `Running`/`Ran`.
    verb: (&'static str, &'static str),
    subject: String,
}

fn describe(name: &str, args: &Value) -> What {
    let text = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("").trim();
    let what = |verb, subject: String| What { verb, subject };
    match name {
        "bash" => {
            let mut subject = one_line(text("command"));
            if !text("workdir").is_empty() {
                subject = format!("{subject} (in {})", text("workdir"));
            }
            what(("Running", "Ran"), subject)
        }
        "read" => {
            let subject = match args.get("offset").and_then(Value::as_u64) {
                Some(offset) => format!("{} from line {offset}", text("path")),
                None => text("path").to_string(),
            };
            what(("Reading", "Read"), subject)
        }
        "edit" if text("old_string").is_empty() => {
            what(("Creating", "Created"), text("path").to_string())
        }
        "edit" => what(("Editing", "Edited"), text("path").to_string()),
        "ask" => {
            let n = args
                .get("questions")
                .and_then(Value::as_array)
                .map_or(1, Vec::len);
            what(("Asking", "Asked"), count(n, "question"))
        }
        "agents" => {
            let (verb, subject) = crate::agents::spec::describe(args);
            what(verb, one_line(&subject))
        }
        "jobs" => {
            let (verb, subject) = crate::jobs::spec::describe(args);
            what(verb, one_line(&subject))
        }
        "window" => window(args),
        _ => what(("Calling", "Called"), name.to_string()),
    }
}

/// The most keys a label lists before it counts the rest.
const KEYS_SHOWN: usize = 6;

/// A call on the window: what it opens, which tab it acts in, and how.
fn window(args: &Value) -> What {
    let what = |verb, subject: String| What { verb, subject };
    let text = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
    let strings = |key: &str| -> Vec<&str> {
        (args
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten())
        .filter_map(Value::as_str)
        .collect()
    };
    let tab = text("tab");
    match text("action") {
        "open" => {
            let one = [args.clone()];
            let tabs = args
                .get("tabs")
                .and_then(Value::as_array)
                .map_or(&one[..], Vec::as_slice);
            let named: Vec<String> = tabs.iter().map(opened).collect();
            what(("Opening", "Opened"), named.join(", "))
        }
        "interact" => interact(args),
        "show" => what(("Showing", "Showed"), format!("tab {tab}")),
        "screenshot" => what(("Picturing", "Pictured"), format!("tab {tab}")),
        "close" => match strings("tabs").as_slice() {
            [] => what(("Closing", "Closed"), format!("tab {tab}")),
            tabs => what(("Closing", "Closed"), format!("tabs {}", tabs.join(", "))),
        },
        "resize" => {
            let size = |key: &str| args.get(key).and_then(Value::as_u64).unwrap_or(0);
            what(
                ("Sizing", "Sized"),
                format!("the window to {}×{}", size("width"), size("height")),
            )
        }
        _ => what(("Listing", "Listed"), "the window's tabs".into()),
    }
}

/// What opening one tab names: its address or command, else its kind.
fn opened(tab: &Value) -> String {
    let text = |key: &str| tab.get(key).and_then(Value::as_str).unwrap_or("");
    [text("url"), text("command")]
        .into_iter()
        .find(|name| !name.is_empty())
        .map_or_else(
            || match text("kind") {
                "" => "a tab".to_string(),
                kind => format!("a {kind} tab"),
            },
            str::to_string,
        )
}

/// An `interact`, by what it does first: a goal handed over, keys pressed or text typed, a
/// script run, an address gone to, a wait, a reframe, else a read.
fn interact(args: &Value) -> What {
    let what = |verb, subject: String| What { verb, subject };
    let text = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
    let keys: Vec<&str> = (args
        .get("keys")
        .and_then(Value::as_array)
        .into_iter()
        .flatten())
    .map(|key| key.as_str().unwrap_or("?"))
    .collect();
    let tab = match text("tab") {
        "" => "the current tab",
        tab => tab,
    };
    if !text("goal").is_empty() {
        return what(
            ("Working in", "Worked in"),
            format!("{tab}: {}", text("goal")),
        );
    }
    let script = !text("script").is_empty();
    // What the page shows the person comes first; a script after it is said as a tail.
    let mut shown = keys
        .iter()
        .take(KEYS_SHOWN)
        .copied()
        .collect::<Vec<_>>()
        .join(" ");
    if keys.len() > KEYS_SHOWN {
        shown = format!("{shown} … ({} keys)", keys.len());
    }
    let mut what = match (text("input").trim(), keys.is_empty()) {
        ("", true) if script => what(("Running a script in", "Ran a script in"), tab.into()),
        ("", true) => return plain(args, tab),
        ("", false) => what(("Pressing", "Pressed"), format!("{shown} in {tab}")),
        (typed, _) => {
            let keys: Vec<String> = keys
                .iter()
                .take(KEYS_SHOWN)
                .map(|k| format!("[{k}]"))
                .collect();
            let typed = [typed.to_string(), keys.join(" ")].join(" ");
            what(
                ("Typing in", "Typed in"),
                format!("{tab}: {}", typed.trim()),
            )
        }
    };
    if script && !(text("input").is_empty() && keys.is_empty()) {
        what.subject.push_str(", then ran a script");
    }
    what
}

/// An `interact` that neither presses, types nor runs: an address, a wait, a reframe, a read.
fn plain(args: &Value, tab: &str) -> What {
    let what = |verb, subject: String| What { verb, subject };
    let text = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
    match (text("url"), text("wait_for"), text("size")) {
        ("", "", "") => what(("Reading", "Read"), tab.into()),
        ("", "", size) => what(("Reframing", "Reframed"), format!("{tab} as {size}")),
        ("", wait, _) => what(("Waiting in", "Waited in"), format!("{tab} for \"{wait}\"")),
        (url, _, _) => what(("Going to", "Went to"), format!("{url} in {tab}")),
    }
}

#[cfg(test)]
#[path = "label_tests.rs"]
mod tests;
