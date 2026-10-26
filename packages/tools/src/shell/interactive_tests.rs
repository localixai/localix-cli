//! A session through the tool: the arguments checked before the person is asked, and what is
//! asked for each call.

use serde_json::{Value, json};

use super::*;
use crate::shell::Sessions;
use crate::testing::{Recorder, Scratch};
use crate::workspace::Workspace;
use contracts::gate::Gate;

fn bash(dir: &Scratch, gate: &dyn Gate, arguments: Value) -> Result<String, ToolError> {
    in_session(&Sessions::default(), dir, gate, arguments)
}

/// A call with the sessions of earlier calls.
#[expect(
    clippy::unwrap_in_result,
    clippy::panic_in_result_fn,
    reason = "a test's helper: a setup that fails, fails the test"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "a test helper takes its arguments as the tests write them, by value"
)]
fn in_session(
    sessions: &Sessions,
    dir: &Scratch,
    gate: &dyn Gate,
    arguments: Value,
) -> Result<String, ToolError> {
    let workspace = Workspace::new(dir.path()).unwrap();
    let tool = BashTool {
        workspace: &workspace,
        gate,
        hidden: &Vec::new,
        stop: &|| false,
        sessions,
    };
    tool.terminal(&arguments).map(|(output, _)| {
        assert_eq!(output.state, None);
        output.report
    })
}

fn failed(result: Result<String, ToolError>) -> String {
    match result {
        Err(ToolError::Failed(message)) => message,
        other => panic!("expected Failed, got {other:?}"),
    }
}

/// The `session` of a running session's report.
#[cfg(unix)]
fn id(report: &str) -> u64 {
    let rest = report
        .strip_prefix("session ")
        .unwrap_or_else(|| panic!("{report}"));
    rest.split(' ').next().unwrap().parse().unwrap()
}

#[test]
fn session_arguments_are_checked_before_the_gate() {
    let dir = Scratch::new();
    let gate = Recorder::default();
    let cases = [
        (
            json!({"command": "x", "session": 1}),
            "pass command to start, or session to continue, not both",
        ),
        (json!({"tty": true}), "the command is empty"),
        (json!({"tty": true, "command": " "}), "the command is empty"),
        (
            json!({"command": "x", "tty": true, "timeout_ms": 5}),
            "timeout_ms is for commands run to their end; a session waits yield_ms per call and \
             ends with close",
        ),
        (
            json!({"command": "x", "input": "y"}),
            "input, keys, wait_for, yield_ms, close and show are for terminal sessions: set tty \
             to start one, or pass session",
        ),
        (
            json!({"command": "x", "close": true}),
            "input, keys, wait_for, yield_ms, close and show are for terminal sessions: set tty \
             to start one, or pass session",
        ),
        (
            json!({"command": "x", "show": true}),
            "input, keys, wait_for, yield_ms, close and show are for terminal sessions: set tty \
             to start one, or pass session",
        ),
        (
            json!({"command": "x", "tty": true, "keys": ["Enter"]}),
            "start the session first, then send input with session",
        ),
        (
            json!({"session": 1, "close": true, "input": "y"}),
            "close ends the session: send input in an earlier call",
        ),
        (
            json!({"session": 1, "keys": ["Foo"]}),
            "unknown key \"Foo\": use Enter, Tab, Escape, Backspace, Space, Up, Down, Left, \
             Right, Home, End, PageUp, PageDown, Delete, F1-F12, or Ctrl-<letter>",
        ),
        (
            json!({"session": 1, "input": "a\n".repeat(2049)}),
            "input is over 4096 bytes: write it to a file and have the program read the file",
        ),
        (
            json!({"session": 1, "input": format!("short\n{}\n", "a".repeat(1001))}),
            "a line of input is over 1000 bytes, and a terminal drops the rest of a longer line, \
             Enter included: break it into lines, or write it to a file and have the program \
             read the file",
        ),
        (
            json!({"session": 1, "input": format!("{}\r{}", "a".repeat(999), "b".repeat(1001))}),
            "a line of input is over 1000 bytes, and a terminal drops the rest of a longer line, \
             Enter included: break it into lines, or write it to a file and have the program \
             read the file",
        ),
        (
            json!({"session": 2, "input": "y"}),
            "session 2 is not running: it ended, or was closed. Running: none",
        ),
    ];
    for (arguments, expected) in cases {
        let message = failed(bash(&dir, &gate, arguments.clone()));
        let expected = match cfg!(unix) {
            true => expected,
            false => NO_TERMINAL,
        };
        assert_eq!(message, expected, "{arguments}");
    }
    assert!(
        gate.asked.lock().unwrap().is_empty(),
        "{:?}",
        gate.asked.lock().unwrap()
    );
}

#[cfg(not(unix))]
const NO_TERMINAL: &str =
    "terminal sessions need a unix system; pass the program's non-interactive flags instead";
#[cfg(unix)]
const NO_TERMINAL: &str = "";

#[cfg(unix)]
#[test]
fn starting_and_typing_ask_but_reading_and_closing_do_not() {
    let (dir, sessions, gate) = (Scratch::new(), Sessions::default(), Recorder::default());
    dir.file("sub/x", "");
    let call = |arguments| in_session(&sessions, &dir, &gate, arguments).unwrap();
    let started = call(json!({"command": "cat", "tty": true, "workdir": "sub", "yield_ms": 100}));
    let session = id(&started);
    call(json!({"session": session, "input": "print(1)\n", "keys": ["Enter", "Ctrl-D"]}));
    assert_eq!(
        *gate.asked.lock().unwrap(),
        [
            "bash: cat  (in a terminal, in sub)".to_string(),
            format!(
                "bash: type into session {session} (`cat`): print(1) \u{23ce} [Enter] [Ctrl-D]"
            ),
        ]
    );
    let other = id(&call(
        json!({"command": "sleep 30", "tty": true, "yield_ms": 100}),
    ));
    gate.asked.lock().unwrap().clear();
    let read = call(json!({"session": other, "yield_ms": 100}));
    // The time is the clock's: a busy machine wakes the wait late, so only its form is checked.
    let waited = read
        .rsplit_once("waited ")
        .and_then(|(_, rest)| rest.strip_suffix("s, no new output"))
        .and_then(|seconds| seconds.parse::<f64>().ok());
    assert!(waited.is_some_and(|seconds| seconds >= 0.1), "{read}");
    let closed = call(json!({"session": other, "close": true}));
    assert!(
        closed.contains("it and everything it started were killed"),
        "{closed}"
    );
    assert!(
        gate.asked.lock().unwrap().is_empty(),
        "{:?}",
        gate.asked.lock().unwrap()
    );
}

#[cfg(unix)]
#[test]
fn a_fifth_session_is_refused_before_the_gate() {
    let (dir, sessions, gate) = (Scratch::new(), Sessions::default(), Recorder::default());
    let start = json!({"command": "sleep 30", "tty": true, "yield_ms": 100});
    for _ in 0..crate::shell::MAX_SESSIONS {
        in_session(&sessions, &dir, &gate, start.clone()).unwrap();
    }
    gate.asked.lock().unwrap().clear();
    let refused = failed(in_session(&sessions, &dir, &gate, start));
    assert!(
        refused.ends_with(": close one first (session, close: true)"),
        "{refused}"
    );
    assert!(gate.asked.lock().unwrap().is_empty());
}

/// What was typed is looked for where it outlives the call: in the file `cat` copies it to, not in
/// the output, which a call that typed before asking would have taken and dropped.
#[expect(
    clippy::disallowed_methods,
    reason = "a test looks again until it is so, for a bounded time: there is no event to wait on"
)]
#[cfg(unix)]
#[test]
fn a_denied_input_types_nothing() {
    let (dir, sessions) = (Scratch::new(), Sessions::default());
    let allow = Recorder::default();
    let started = in_session(
        &sessions,
        &dir,
        &allow,
        json!({"command": "cat > typed.txt", "tty": true, "yield_ms": 100}),
    );
    let session = id(&started.unwrap());
    let deny = Recorder::denying("no");
    let typed = json!({"session": session, "input": "secret-words\n"});
    let refused = failed(in_session(&sessions, &dir, &deny, typed));
    assert_eq!(refused, "not allowed: no");
    let allowed = json!({"session": session, "input": "allowed-words\n", "yield_ms": 300});
    in_session(&sessions, &dir, &allow, allowed).unwrap();
    // `cat` copies each line as it reads it: once the allowed line is there, so is any before it.
    let file = dir.path().join("typed.txt");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let copied = loop {
        let copied = std::fs::read_to_string(&file).unwrap_or_default();
        if copied.contains("allowed-words") || std::time::Instant::now() > deadline {
            break copied;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(copied, "allowed-words\n", "the denied input was typed");
}

#[cfg(unix)]
#[test]
fn a_line_as_long_as_allowed_reaches_the_program() {
    let (dir, sessions, gate) = (Scratch::new(), Sessions::default(), Recorder::default());
    let call = |arguments| in_session(&sessions, &dir, &gate, arguments).unwrap();
    let started = call(
        json!({"command": "IFS= read -r line; echo \"n=${#line}\"; read rest",
        "tty": true, "yield_ms": 300}),
    );
    let line = format!("{}\n", "a".repeat(1000));
    let read = call(json!({"session": id(&started), "input": line, "wait_for": "n=1000"}));
    assert!(read.contains("\"n=1000\" appeared after"), "{read}");
}

#[cfg(unix)]
#[test]
fn a_session_starts_and_ends_through_the_tool() {
    let (dir, sessions, gate) = (Scratch::new(), Sessions::default(), Recorder::default());
    let call = |arguments| in_session(&sessions, &dir, &gate, arguments).unwrap();
    let started = call(json!({"command": "printf 'Name: '; read n; echo \"hi $n\"",
        "tty": true, "wait_for": "Name:"}));
    assert!(started.contains("\"Name:\" appeared after"), "{started}");
    assert!(started.ends_with("### output\nName:"), "{started}");
    let ended = call(json!({"session": id(&started), "input": "bob\n"}));
    assert!(ended.starts_with("exit 0 after "), "{ended}");
    assert!(ended.contains("hi bob"), "{ended}");
    let quick = call(json!({"command": "echo done", "tty": true}));
    assert!(quick.starts_with("exit 0 in "), "{quick}");
    assert!(
        quick.contains("s (in a terminal)\n### output\ndone"),
        "{quick}"
    );
}

#[cfg(windows)]
#[test]
fn a_terminal_is_refused_on_windows() {
    let dir = Scratch::new();
    let message = failed(bash(
        &dir,
        &Recorder::default(),
        json!({"command": "x", "tty": true}),
    ));
    assert_eq!(message, NO_TERMINAL);
}

/// Sessions that open a window only when a call asks, each window keeping its frames in `frames`.
#[cfg(unix)]
fn watched(frames: &std::sync::Arc<std::sync::Mutex<Vec<String>>>) -> Sessions {
    let frames = std::sync::Arc::clone(frames);
    let watch = move |_: u32, _: &str| {
        let frames = std::sync::Arc::clone(&frames);
        let painter = move |frame: String| {
            frames.lock().unwrap().push(frame);
            true
        };
        Some(Box::new(painter) as crate::shell::Painter)
    };
    Sessions::new(std::sync::Arc::new(watch), false)
}

/// A session driven through the tool, with `show` on every call or on none: what the gate was
/// asked, what the model read (each report's wait time cut), and the frames painted.
#[expect(
    clippy::disallowed_methods,
    reason = "a test looks again until it is so, for a bounded time: there is no event to wait on"
)]
#[cfg(unix)]
fn python(show: bool) -> (Vec<String>, Vec<String>, Vec<String>) {
    let frames = std::sync::Arc::default();
    let (dir, sessions, gate) = (Scratch::new(), watched(&frames), Recorder::default());
    let call = |mut arguments: Value| {
        if show {
            arguments["show"] = json!(true);
        }
        let report = in_session(&sessions, &dir, &gate, arguments).unwrap();
        match report.split_once(" after ") {
            Some((head, rest)) => format!(
                "{head}{}",
                rest.get(rest.find('\n').unwrap_or(0)..).unwrap()
            ),
            None => report,
        }
    };
    let started = call(json!({"command": "python3 -q", "tty": true, "wait_for": ">>> "}));
    let typed =
        call(json!({"session": id(&started), "input": "print(1+1)\n", "wait_for": "2\n>>> "}));
    // The last frame is painted within a frame's time of the output, after the call returned.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while show && !frames.lock().unwrap().iter().any(|frame| sum(frame)) {
        assert!(std::time::Instant::now() < deadline, "{:#?}", frames.lock());
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    call(json!({"session": id(&started), "close": true}));
    let asked = gate.asked.lock().unwrap().clone();
    let frames = frames.lock().unwrap().clone();
    (asked, vec![started, typed], frames)
}

/// A frame that shows the sum typed and its answer, each on a row of its own.
#[cfg(unix)]
fn sum(frame: &str) -> bool {
    frame.contains(r#"">>> print(1+1)","2""#)
}

#[cfg(unix)]
#[test]
fn show_puts_a_session_in_a_window_and_changes_nothing_the_model_or_the_person_reads() {
    let (asked, reports, frames) = python(true);
    let (plain_asked, plain_reports, plain_frames) = python(false);
    assert_eq!(asked, plain_asked, "the gate is asked the same");
    assert_eq!(reports, plain_reports, "the model reads the same");
    assert!(plain_frames.is_empty(), "no window unless asked");
    assert!(frames.iter().any(|frame| sum(frame)), "{frames:#?}");
}
