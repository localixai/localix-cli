//! A session's prompt, in the interface: `llx` at a terminal, after its first message when one was
//! given. Each entry is the next task, in the same conversation, workspace and
//! session. `/login` chooses the provider, model and reasoning level, `/new`
//! starts over, `/status` says where things stand, `/mode` shows or switches the mode, and
//! Ctrl-D or `/exit` quits. The time limit is per task, so waiting for the person to type never
//! times out.

use crate::commands::{login, model};
use crate::compose::Rig;
use crate::exit;
use crate::mode;
use crate::server::Ui;
use protocol::Event;

pub(crate) use super::entry::{Entry, HELP, commands, entry};

/// The session in `ui`, after `first` when it is given.
pub(crate) fn run(rig: &mut Rig, ui: &Ui, first: Option<&str>) -> i32 {
    ui.send(Event::Mode(rig.mode.get().name().to_string()));
    ui.send(Event::Header(vec![
        env!("CARGO_PKG_VERSION").to_string(),
        rig.choice.label(),
        super::sessions::home_relative(&rig.workspace.root().display().to_string()),
    ]));
    if !rig.session.turns.is_empty() {
        let turns = rig.session.turns.iter().map(crate::server::wire::turn);
        ui.send(Event::Earlier(turns.collect()));
    }
    let signed_out = login::signed_out(&rig.choice, rig.workspace.env());
    if !signed_out.is_empty() {
        ui.send(Event::Note(signed_out));
    }
    if let Some(task) = first
        && !attempt(rig, task)
    {
        return exit::OK;
    }
    while let Some(typed) = ui.next_entry() {
        if !typed.images.is_empty() {
            rig.attached = typed.images;
        }
        match entry(&typed.text) {
            None => {}
            Some(Entry::Exit) => return exit::OK,
            Some(Entry::Login) => login::run(rig, ui),
            Some(Entry::Model) => model::run_model(rig, ui),
            Some(Entry::Rewind) => crate::commands::rewind::run(rig, ui),
            Some(entry) => {
                if let Some(task) = command(rig, entry)
                    && !attempt(rig, &task)
                {
                    return exit::OK;
                }
            }
        }
    }
    exit::OK
}

/// Carry out an entry that is not `/exit`; the task it asks to run, if any.
fn command(rig: &mut Rig, entry: Entry) -> Option<String> {
    let note = |text: &str| rig.output.note(text);
    match entry {
        Entry::Task(task) => return Some(task),
        Entry::Help => note(HELP),
        Entry::Diff => note(&super::extras::diff(rig.workspace.root())),
        Entry::Copy => {
            if let Some(said) = super::extras::copy(rig) {
                note(said);
            }
        }
        Entry::Usage => note(&super::extras::usage(rig)),
        Entry::Status => note(&status(rig)),
        Entry::Modes => note(&mode::list(&rig.mode.get(), rig.modes.all())),
        Entry::Switch(name) => match rig.modes.named(&name) {
            Some(mode) => {
                note(&mode::switched(&mode));
                rig.switch(mode);
            }
            None => note(&format!("unknown mode {name}: {}\n", rig.modes.names())),
        },
        Entry::Reasoning(up) => {
            let said = model::step_reasoning(rig, up);
            note(&said);
        }
        Entry::CycleMode => {
            let mode = rig.modes.after(&rig.mode.get());
            note(&mode::switched(&mode));
            rig.switch(mode);
        }

        Entry::Unknown(command) => {
            note(&format!("unknown command /{command}: /help lists them\n"));
        }
        // Each is carried out in `run`, where the interface is at hand; the interface itself
        // opens a new lane (the desk does), whatever this one is doing.
        Entry::Exit | Entry::Login | Entry::Model | Entry::Rewind | Entry::New => {}
    }
    None
}

/// Run `task`. False once the output's reader is gone.
fn attempt(rig: &mut Rig, task: &str) -> bool {
    rig.task(task);
    !rig.output.closed()
}

/// `/status`: what the next task runs with.
pub(crate) fn status(rig: &Rig) -> String {
    format!(
        "model      {} ({})\nworkspace  {}\nmode       {}\nsession    {} ({} tasks)\n",
        rig.choice.label(),
        login::signed(&rig.choice, rig.workspace.env()),
        rig.workspace.root().display(),
        rig.mode.get().name(),
        rig.session.id,
        rig.session.turns.len(),
    )
}

#[cfg(test)]
#[path = "repl_tests.rs"]
mod tests;
