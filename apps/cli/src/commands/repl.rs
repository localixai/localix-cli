//! The prompt line: `lx` at a terminal, after its first message when one was given. Each line is
//! the next task, in the same conversation, workspace, browser window and session; a line ending
//! in `\` goes on on the next. `/new` starts over, `/status` says where things stand, `/mode`
//! shows or switches the mode, `/plan` enters plan mode, and Ctrl-D or `/exit` quits. A plan
//! that ends well is offered to be carried out. The time limit is per task, so waiting for the
//! person to type never times out.

use std::io::{BufRead, Write};

use crate::compose::Rig;
use crate::exit;
use crate::mode::{self, Mode};

const HELP: &str = "Type a task and press Enter; each one goes on from the last. End a line \
with \\ to go on on the next.
  /mode [name]      the modes, or switch to one (also /permissions)
  /plan [task]      plan mode: investigate and answer with a plan; with a task, run it
  Shift+Tab, Enter  plan mode on or off
  /new              start a new conversation and session (also /clear)
  /status           the model, the workspace, the mode, the session and its terminals
  /help             this help
  /exit             quit (also /quit, or Ctrl-D)\n";

/// What Shift+Tab types. The line is read in the terminal's cooked mode, so the key reaches lx only
/// with Enter after it: a single-key toggle would need a line editor of its own.
const SHIFT_TAB: &str = "\u{1b}[Z";

/// The task that carries out a plan, in Codex's words.
const IMPLEMENT: &str = "Implement the plan.";

/// One entry at the prompt line.
#[derive(Debug, PartialEq, Eq)]
pub enum Entry {
    Task(String),
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

/// What comes after a task.
enum Next {
    Prompt,
    /// An entry already typed.
    Entry(String),
    /// The output's reader is gone.
    Quit,
}

/// The prompt line, after `first` when it is given.
pub fn run(rig: &mut Rig, first: Option<&str>) -> i32 {
    let mode = match rig.mode.get() {
        Mode::Default => String::new(),
        mode => format!(" · {}", mode.name()),
    };
    rig.output.note(&format!(
        "lx {} · {} · {}{mode}\n{}",
        env!("CARGO_PKG_VERSION"),
        rig.options.models.model,
        rig.workspace.root().display(),
        "Type a task; each one goes on from the last. /help, or Ctrl-D to quit.\n"
    ));
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let shown = |text: &str| {
        let mut err = std::io::stderr();
        let _ = err.write_all(text.as_bytes()).and_then(|()| err.flush());
    };
    let mut next = match first {
        Some(task) => attempt(rig, task, &mut input, &shown),
        None => Next::Prompt,
    };
    loop {
        let text = match std::mem::replace(&mut next, Next::Prompt) {
            Next::Quit => return exit::OK,
            Next::Entry(text) => text,
            Next::Prompt => match read(&mut input, &shown, &prompt(rig.mode.get())) {
                Some(text) => text,
                None => {
                    shown("\n");
                    return exit::OK;
                }
            },
        };
        match entry(&text) {
            None => {}
            Some(Entry::Exit) => return exit::OK,
            Some(entry) => {
                if let Some(task) = command(rig, entry) {
                    next = attempt(rig, &task, &mut input, &shown);
                }
            }
        }
    }
}

/// Carry out an entry that is not `/exit`; the task it asks to run, if any.
fn command(rig: &mut Rig, entry: Entry) -> Option<String> {
    let note = |text: &str| rig.output.note(text);
    match entry {
        Entry::Task(task) => return Some(task),
        Entry::Plan(task) => {
            rig.switch(Mode::Plan);
            note(&mode::switched(Mode::Plan));
            return task;
        }
        Entry::Help => note(HELP),
        Entry::Status => note(&status(rig)),
        Entry::Modes => note(&mode::list(rig.mode.get())),
        Entry::Switch(mode) => {
            rig.switch(mode);
            note(&mode::switched(mode));
        }
        Entry::TogglePlan => {
            let mode = match rig.mode.get() {
                Mode::Plan => rig.before_plan,
                _ => Mode::Plan,
            };
            rig.switch(mode);
            note(&mode::switched(mode));
        }
        Entry::UnknownMode(name) => note(&format!(
            "unknown mode {name}: {}\n",
            Mode::ALL.map(Mode::name).join(", ")
        )),
        Entry::New => {
            rig.start_over();
            note(&format!(
                "A new conversation, session {}.\n",
                rig.session.id
            ));
        }
        Entry::Unknown(command) => {
            note(&format!("unknown command /{command}: /help lists them\n"));
        }
        Entry::Exit => {}
    }
    None
}

/// Run `task`; when it was a plan that ended well, ask whether to carry it out.
fn attempt(rig: &mut Rig, task: &str, input: &mut dyn BufRead, shown: &dyn Fn(&str)) -> Next {
    let code = rig.task(task);
    if rig.output.closed() {
        return Next::Quit;
    }
    if code != exit::OK || rig.mode.get() != Mode::Plan {
        return Next::Prompt;
    }
    let question = format!(
        "\n  Implement this plan? [y]es, in {} mode  [n]o, stay in plan  (or type to revise it) > ",
        rig.before_plan.name()
    );
    match handoff(read(input, shown, &question).as_deref()) {
        Handoff::Stay => Next::Prompt,
        Handoff::Next(text) => Next::Entry(text),
        Handoff::Implement => {
            let mode = rig.before_plan;
            rig.switch(mode);
            rig.output.note(&mode::switched(mode));
            // The plan is in the conversation already: this only says to carry it out.
            rig.task(IMPLEMENT);
            match rig.output.closed() {
                true => Next::Quit,
                false => Next::Prompt,
            }
        }
    }
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

/// `/status`: what the next task runs with, and the terminal sessions `bash` keeps (each task
/// closes its own as it ends, so this shows one only when something was left behind).
pub fn status(rig: &Rig) -> String {
    format!(
        "model      {}\ndecision   {}\nworkspace  {}\nmode       {}\nsession    {} ({} tasks)\n\
         terminals  {}\n",
        rig.options.models.model,
        rig.options.models.decision_model,
        rig.workspace.root().display(),
        rig.mode.get().name(),
        rig.session.id,
        rig.session.turns.len(),
        rig.sessions.running(),
    )
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
        ("new" | "clear", "") => Entry::New,
        ("status", "") => Entry::Status,
        ("help" | "?", "") => Entry::Help,
        _ => Entry::Unknown(command.to_string()),
    })
}

#[cfg(test)]
#[path = "repl_tests.rs"]
mod tests;
