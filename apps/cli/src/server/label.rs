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

/// A call on the window: what it opens, which tab it acts in, and how.
fn window(args: &Value) -> What {
    let text = |value: &Value, key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let what = |verb, subject: String| What { verb, subject };
    let tab = text(args, "tab");
    match text(args, "action").as_str() {
        "open" => {
            let one = [args.clone()];
            let tabs = args
                .get("tabs")
                .and_then(Value::as_array)
                .map_or(&one[..], Vec::as_slice);
            let named: Vec<String> = (tabs.iter())
                .map(|tab| match text(tab, "url").as_str() {
                    "" => text(tab, "command"),
                    url => url.to_string(),
                })
                .filter(|name| !name.is_empty())
                .collect();
            what(("Opening", "Opened"), named.join(", "))
        }
        "interact" if !text(args, "goal").is_empty() => what(
            ("Working in", "Worked in"),
            format!("{tab}: {}", text(args, "goal")),
        ),
        "interact" => {
            let keys = (args
                .get("keys")
                .and_then(Value::as_array)
                .into_iter()
                .flatten())
            .map(|key| format!("[{}]", key.as_str().unwrap_or("?")));
            let mut typed: Vec<String> = vec![text(args, "input")];
            typed.extend(keys);
            let typed = typed.join(" ").trim().to_string();
            match typed.is_empty() {
                true => what(("Reading", "Read"), tab),
                false => what(("Typing in", "Typed in"), format!("{tab}: {typed}")),
            }
        }
        "show" => what(("Showing", "Showed"), format!("tab {tab}")),
        "screenshot" => what(("Picturing", "Pictured"), format!("tab {tab}")),
        "close" => {
            let tabs: Vec<&str> = (args
                .get("tabs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten())
            .filter_map(Value::as_str)
            .collect();
            match tabs.is_empty() {
                true => what(("Closing", "Closed"), format!("tab {tab}")),
                false => what(("Closing", "Closed"), format!("tabs {}", tabs.join(", "))),
            }
        }
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

#[cfg(test)]
#[path = "label_tests.rs"]
mod tests;
