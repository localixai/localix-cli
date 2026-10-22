//! The prompt line: `llx` at a terminal, after its first message when one was given. Each line is
//! the next task, in the same conversation, workspace, browser window and session; a line ending
//! in `\` goes on on the next. `/login` chooses the provider, model and reasoning level, `/new`
//! starts over, `/status` says where things stand, `/mode` shows or switches the mode, `/plan`
//! enters plan mode, and Ctrl-D or `/exit` quits. A plan that ends well is offered to be carried
//! out. The time limit is per task, so waiting for the person to type never times out.

use std::io::{BufRead, Write};

use crate::commands::login;
use crate::compose::Rig;
use crate::exit;
use crate::mode::{self, Mode};
use crate::tui::event::Event;

pub use super::entry::{Entry, HELP, Handoff, commands, entry, handoff, prompt, read};

/// The task that carries out a plan, in Codex's words.
const IMPLEMENT: &str = "Implement the plan.";

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
    if let Some(ui) = &rig.ui {
        ui.send(Event::Mode(rig.mode.get().name().to_string()));
        ui.send(Event::Header(vec![
            env!("CARGO_PKG_VERSION").to_string(),
            rig.choice.label(),
            home_relative(&rig.workspace.root().display().to_string()),
        ]));
        let signed_out = login::signed_out(&rig.choice);
        if !signed_out.is_empty() {
            ui.send(Event::Note(signed_out));
        }
    }
    let mode = match rig.mode.get() {
        Mode::Default => String::new(),
        mode => format!(" · {}", mode.name()),
    };
    if rig.ui.is_none() {
        rig.output.note(&format!(
            "llx {} · {} · {}{mode}\n{}{}",
            env!("CARGO_PKG_VERSION"),
            rig.choice.label(),
            rig.workspace.root().display(),
            login::signed_out(&rig.choice),
            "Type a task; each one goes on from the last. /help, or Ctrl-D to quit.\n"
        ));
    }
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
            Next::Prompt => match ask_entry(rig, &mut input, &shown) {
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
            Some(Entry::Login) => login::run(rig, &mut input, &shown),
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
        // Both are carried out where the prompt line is read.
        Entry::Exit | Entry::Login => {}
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
    let answer = match &rig.ui {
        Some(ui) => {
            let yes = format!("Yes, in {} mode", rig.before_plan.name());
            let picked = ui.choose("Implement this plan?", &[&yes, "No, stay in plan mode"]);
            Some(if picked == Some(0) { "y" } else { "n" }.to_string())
        }
        None => {
            let question = format!(
                "\n  Implement this plan? [y]es, in {} mode  [n]o, stay in plan  (or type to revise it) > ",
                rig.before_plan.name()
            );
            read(input, shown, &question)
        }
    };
    match handoff(answer.as_deref()) {
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

/// The next entry: from the interface, or read after the prompt.
fn ask_entry(rig: &mut Rig, input: &mut dyn BufRead, shown: &dyn Fn(&str)) -> Option<String> {
    match rig.ui.clone() {
        Some(ui) => {
            let typed = ui.next_entry()?;
            if !typed.images.is_empty() {
                rig.attached = typed.images;
            }
            Some(typed.text)
        }
        None => read(input, shown, &prompt(rig.mode.get())),
    }
}

/// `~/x` for a path under the home folder.
fn home_relative(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && path.starts_with(&home) => {
            format!("~{}", &path[home.len()..])
        }
        _ => path.to_string(),
    }
}

/// `/status`: what the next task runs with, and the terminal sessions `bash` keeps (each task
/// closes its own as it ends, so this shows one only when something was left behind).
pub fn status(rig: &Rig) -> String {
    format!(
        "model      {} ({})\ndecision   {}\nworkspace  {}\nmode       {}\nsession    {} ({} tasks)\n\
         terminals  {}\n",
        rig.choice.label(),
        login::signed(&rig.choice),
        rig.options.models.decision_model,
        rig.workspace.root().display(),
        rig.mode.get().name(),
        rig.session.id,
        rig.session.turns.len(),
        rig.sessions.running(),
    )
}

#[cfg(test)]
#[path = "repl_tests.rs"]
mod tests;
