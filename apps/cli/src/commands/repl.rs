//! A session's prompt, in the interface: `llx` at a terminal, after its first message when one was
//! given. Each entry is the next task, in the same conversation, workspace, browser window and
//! session. `/login` chooses the provider, model and reasoning level, `/new`
//! starts over, `/status` says where things stand, `/mode` shows or switches the mode, `/plan`
//! enters plan mode, and Ctrl-D or `/exit` quits. A plan that ends well is offered to be carried
//! out. The time limit is per task, so waiting for the person to type never times out.

use crate::commands::{login, model};
use crate::compose::Rig;
use crate::exit;
use crate::mode::{self, Mode};
use crate::server::Ui;
use protocol::Event;

pub(crate) use super::entry::{Entry, HELP, commands, entry};

/// The task that carries out a plan.
const IMPLEMENT: &str = "Implement the plan.";

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
    let signed_out = login::signed_out(&rig.choice);
    if !signed_out.is_empty() {
        ui.send(Event::Note(signed_out));
    }
    if let Some(task) = first
        && !attempt(rig, ui, task)
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
                    && !attempt(rig, ui, &task)
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
        Entry::Plan(task) => {
            rig.switch(Mode::Plan);
            note(&mode::switched(Mode::Plan));
            return task;
        }
        Entry::Help => note(HELP),
        Entry::Diff => note(&super::extras::diff(rig.workspace.root())),
        Entry::Copy => note(&super::extras::copy(rig)),
        Entry::Usage => note(&super::extras::usage(rig)),
        Entry::Status => note(&status(rig)),
        Entry::Modes => note(&mode::list(rig.mode.get())),
        Entry::Switch(mode) => {
            rig.switch(mode);
            note(&mode::switched(mode));
        }
        Entry::Reasoning(up) => {
            let said = model::step_reasoning(rig, up);
            note(&said);
        }
        Entry::CycleMode => {
            let current = Mode::ALL
                .iter()
                .position(|&mode| mode == rig.mode.get())
                .unwrap_or(0);
            let mode = Mode::ALL[(current + 1) % Mode::ALL.len()];
            rig.switch(mode);
            note(&mode::switched(mode));
        }
        Entry::UnknownMode(name) => note(&format!(
            "unknown mode {name}: {}\n",
            Mode::ALL.map(Mode::name).join(", ")
        )),

        Entry::Unknown(command) => {
            note(&format!("unknown command /{command}: /help lists them\n"));
        }
        // Each is carried out in `run`, where the interface is at hand; the interface itself
        // opens a new lane (the desk does), whatever this one is doing.
        Entry::Exit | Entry::Login | Entry::Model | Entry::Rewind | Entry::New => {}
    }
    None
}

/// Run `task`; when it was a plan that ended well, ask whether to carry it out. False once the
/// output's reader is gone.
fn attempt(rig: &mut Rig, ui: &Ui, task: &str) -> bool {
    let code = rig.task(task);
    if rig.output.closed() {
        return false;
    }
    if code != exit::OK || rig.mode.get() != Mode::Plan {
        return true;
    }
    let yes = format!("Yes, in {} mode", rig.before_plan.name());
    let options = [yes, "No, stay in plan mode".into()].map(|o| (o, String::new()));
    if ui.pick("Implement this plan?", options.into(), None) != Some(0) {
        return true;
    }
    let mode = rig.before_plan;
    rig.switch(mode);
    rig.output.note(&mode::switched(mode));
    // The plan is in the conversation already: this only says to carry it out.
    rig.task(IMPLEMENT);
    !rig.output.closed()
}

/// `/status`: what the next task runs with, and the terminal sessions `bash` keeps (each task
/// closes its own as it ends, so this shows one only when something was left behind).
pub(crate) fn status(rig: &Rig) -> String {
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
