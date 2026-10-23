//! What a session call says: to the person, what will be typed; to the model, how the wait ended
//! and the new output.

use std::time::Duration;

use super::process::Ending;
use super::session::{Turn, Until};

/// What is typed, as the person approves it: the text with each line break as a return sign,
/// then each key in brackets.
pub fn typed(input: &str, keys: &[String]) -> String {
    let mut text = input.replace("\r\n", "\n").replace('\n', " \u{23ce} ");
    if input.ends_with('\n') {
        text.pop();
    }
    let keys = keys.iter().map(|key| format!("[{key}]"));
    let parts: Vec<String> = [text]
        .into_iter()
        .chain(keys)
        .filter(|part| !part.is_empty())
        .collect();
    parts.join(" ")
}

/// The call as the model reads it: how the wait ended, then the new output. `place` is where
/// a session was started, for a start's report.
pub fn report(turn: &Turn, place: Option<&str>, needle: Option<&str>, input: &str) -> String {
    let (id, command) = (turn.id, &turn.command);
    let (waited, age) = (seconds(turn.waited), seconds(turn.age));
    let running = format!("session {id} running (`{command}`)");
    let mut head = match (turn.until, needle) {
        (Until::Matched, Some(needle)) => {
            let mut head = format!("{running}: \"{needle}\" appeared after {waited}");
            if input.contains(needle) {
                head.push_str("; the text was also in the input, so this may be its echo");
            }
            head
        }
        (Until::Deadline, Some(needle)) => format!(
            "{running}: \"{needle}\" did not appear within {waited}; it may still be working: \
             read again with session {id}, or close it"
        ),
        (Until::Matched | Until::Settled | Until::Deadline, _) => {
            format!("{running}, waited {waited}")
        }
        (Until::Exited(ending), _) => match place {
            Some(place) => format!("{} in {age} ({place})", ended(ending)),
            None => format!(
                "{} after {age}; session {id} ended (`{command}`)",
                ended(ending)
            ),
        },
        (Until::Closed { killed }, _) => format!(
            "session {id} closed after {age} (`{command}`): {}",
            match killed {
                true => "it and everything it started were killed",
                false => "it could not be killed, and may still be running",
            }
        ),
        (Until::Stopped, _) => {
            format!("stopped waiting after {waited}: session {id} still runs")
        }
    };
    let mut out = vec![];
    match &turn.output {
        Some(output) => out.push(format!("### output\n{output}")),
        None => head.push_str(", no new output"),
    }
    out.insert(0, head);
    if let Some((sent, all)) = turn.stalled {
        out.push(format!(
            "(the program is not reading its input: {sent} of {all} bytes sent)"
        ));
    }
    out.join("\n")
}

fn ended(ending: Ending) -> String {
    match ending {
        Ending::Exit(code) => format!("exit {code}"),
        Ending::Signal(signal) => format!("killed by signal {signal}"),
        _ => "ended".into(),
    }
}

fn seconds(time: Duration) -> String {
    format!("{:.1}s", time.as_secs_f64())
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
