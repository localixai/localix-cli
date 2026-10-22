//! What is typed at a session's prompt: the slash commands, the help, the answer to
//! "Implement this plan?", and the plain prompt line used where the interface cannot draw.

use std::io::BufRead;

use crate::mode::Mode;

pub const HELP: &str = "Type a task and press Enter; each one goes on from the last.
  /login            sign in, and choose the provider, the model and its reasoning level
  /mode [name]      the modes, or switch to one (also /permissions)
  /plan [task]      plan mode: investigate and answer with a plan; with a task, run it
  /new              start a new conversation and session (also /clear)
  /status           the model, the workspace, the mode, the session and its terminals
  /help             this help
  /exit             quit (also /quit, or Ctrl-D)
Keys
  Shift+Enter       a new line (also Ctrl-J, or a line ending in \\)
  Up, Down          earlier entries
  Tab               complete a command
  Shift+Tab         plan mode on or off
  Esc               stop the task (a message typed meanwhile waits for it)
  Ctrl-C            stop the task, clear the line, or twice to quit\n";

/// What Shift+Tab sends: on the plain prompt line it reaches llx with Enter after it.
pub const SHIFT_TAB: &str = "\u{1b}[Z";

/// One entry at the prompt line.
#[derive(Debug, PartialEq, Eq)]
pub enum Entry {
    Task(String),
    Login,
    New,
    Status,
    Help,
    Exit,
    /// `/mode`: the modes, the current one marked.
    Modes,
    /// `/mode <name>`.
    Switch(Mode),
    /// `/mode` with a name that is not one.
    UnknownMode(String),
    /// `/plan`, with a task to run in plan mode.
    Plan(Option<String>),
    /// Shift+Tab, then Enter.
    TogglePlan,
    /// A `/command` that is not one.
    Unknown(String),
}

/// What the person answered to "Implement this plan?".
#[derive(Debug, PartialEq, Eq)]
pub enum Handoff {
    Implement,
    Stay,
    /// Something else: the next entry (a revision of the plan, or a command).
    Next(String),
}

/// What `/` offers in the interface: each command and what it does.
pub fn commands() -> Vec<(String, String)> {
    [
        ("help", "the commands and keys"),
        (
            "login",
            "sign in; choose the provider, the model and its reasoning",
        ),
        ("mode", "show the modes, or switch: /mode plan"),
        ("plan", "plan mode: investigate and answer with a plan"),
        ("new", "start a new conversation and session"),
        ("status", "the model, the workspace, the mode, the session"),
        ("exit", "quit"),
    ]
    .into_iter()
    .map(|(name, what)| (name.to_string(), what.to_string()))
    .collect()
}

/// The answer to "Implement this plan?": yes, no (also Enter, or the end of input), or anything
/// else as the next entry.
pub fn handoff(line: Option<&str>) -> Handoff {
    let Some(line) = line.map(str::trim) else {
        return Handoff::Stay;
    };
    match line.to_ascii_lowercase().as_str() {
        "y" | "yes" => Handoff::Implement,
        "" | "n" | "no" => Handoff::Stay,
        _ => Handoff::Next(line.to_string()),
    }
}

/// The prompt, with the mode when it is not the default.
pub fn prompt(mode: Mode) -> String {
    match mode {
        Mode::Default => "\n› ".into(),
        mode => format!("\n{} › ", mode.name()),
    }
}

/// The next entry's text after `prompt`, joined over lines that end in `\`; `None` at the end of
/// input, or when it cannot be read (said on `shown`). A line that is not UTF-8 (pasted from
/// another program) is read with its bad bytes replaced, not taken for the end.
pub fn read(input: &mut dyn BufRead, shown: &dyn Fn(&str), prompt: &str) -> Option<String> {
    let mut text = String::new();
    shown(prompt);
    loop {
        let mut bytes = Vec::new();
        // `read_until` already retries a read a signal interrupted.
        match input.read_until(b'\n', &mut bytes) {
            Ok(0) => return (!text.is_empty()).then_some(text),
            Ok(_) => {}
            Err(error) => {
                shown(&format!(
                    "\nlx: the prompt line could not be read: {error}\n"
                ));
                return None;
            }
        }
        let line = String::from_utf8_lossy(&bytes);
        let line = line.trim_end_matches(['\n', '\r']);
        match line.strip_suffix('\\') {
            Some(start) => {
                text.push_str(start);
                text.push('\n');
                shown("  ");
            }
            None => {
                text.push_str(line);
                return Some(text);
            }
        }
    }
}

/// What `text` asks for; `None` for a blank line.
pub fn entry(text: &str) -> Option<Entry> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if text == SHIFT_TAB {
        return Some(Entry::TogglePlan);
    }
    let Some(command) = text.strip_prefix('/') else {
        return Some(Entry::Task(text.to_string()));
    };
    let command = command.trim();
    let (name, rest) = command
        .split_once(char::is_whitespace)
        .map_or((command, ""), |(name, rest)| (name, rest.trim()));
    Some(match (name, rest) {
        ("mode" | "permissions", "") => Entry::Modes,
        ("mode" | "permissions", name) => match Mode::named(name) {
            Some(mode) => Entry::Switch(mode),
            None => Entry::UnknownMode(name.to_string()),
        },
        ("plan", "") => Entry::Plan(None),
        ("plan", task) => Entry::Plan(Some(task.to_string())),
        ("exit" | "quit", "") => Entry::Exit,
        ("login", "") => Entry::Login,
        ("new" | "clear", "") => Entry::New,
        ("status", "") => Entry::Status,
        ("help" | "?", "") => Entry::Help,
        _ => Entry::Unknown(command.to_string()),
    })
}
