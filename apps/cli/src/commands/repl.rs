//! The prompt line: `lx` with no prompt, at a terminal. Each line is the next task, in the
//! same conversation, workspace, browser window and session; a line ending in `\` goes on on the
//! next. Ctrl-D or `/exit` quits. The time limit is per task, so waiting for the person to type
//! never times out.

use std::io::{BufRead, Write};

use crate::compose::Rig;
use crate::exit;

const HELP: &str = "Type a task and press Enter; each one goes on from the last. End a line \
with \\ to go on on the next.\n  /help   this help\n  /exit   quit (or Ctrl-D)\n";

/// One entry at the prompt line.
#[derive(Debug, PartialEq, Eq)]
pub enum Entry {
    Task(String),
    Help,
    Exit,
    /// A `/command` that is not one.
    Unknown(String),
}

pub fn run(rig: &mut Rig) -> i32 {
    rig.output.note(&format!(
        "lx {} · {} · {}\n{}",
        env!("CARGO_PKG_VERSION"),
        rig.options.models.model,
        rig.workspace.root().display(),
        "Type a task; each one goes on from the last. /help, or Ctrl-D to quit.\n"
    ));
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    loop {
        let shown = |text: &str| {
            let mut err = std::io::stderr();
            let _ = err.write_all(text.as_bytes()).and_then(|()| err.flush());
        };
        let Some(text) = read(&mut input, &shown) else {
            shown("\n");
            return exit::OK;
        };
        match entry(&text) {
            None => {}
            Some(Entry::Exit) => return exit::OK,
            Some(Entry::Help) => rig.output.note(HELP),
            Some(Entry::Unknown(command)) => rig
                .output
                .note(&format!("unknown command /{command}: /help lists them\n")),
            Some(Entry::Task(task)) => {
                rig.task(&task);
                if rig.output.closed() {
                    return exit::OK;
                }
            }
        }
    }
}

/// The next entry's text, joined over lines that end in `\`; `None` at the end of input, or when
/// it cannot be read (said on `shown`). A line that is not UTF-8 (pasted from another program) is
/// read with its bad bytes replaced, not taken for the end.
pub fn read(input: &mut dyn BufRead, shown: &dyn Fn(&str)) -> Option<String> {
    let mut text = String::new();
    shown("\n› ");
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
    let Some(command) = text.strip_prefix('/') else {
        return Some(Entry::Task(text.to_string()));
    };
    Some(match command.trim() {
        "exit" | "quit" => Entry::Exit,
        "help" | "?" => Entry::Help,
        other => Entry::Unknown(other.to_string()),
    })
}

#[cfg(test)]
#[path = "repl_tests.rs"]
mod tests;
