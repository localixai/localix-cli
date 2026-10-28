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
        _ => what(("Calling", "Called"), name.to_string()),
    }
}

#[cfg(test)]
#[path = "label_tests.rs"]
mod tests;
