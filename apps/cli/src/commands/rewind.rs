//! `/rewind` (also `/undo`, or Esc Esc on an empty prompt): back to before
//! one of the conversation's tasks. The files as they were then (what `apply_patch` changed), the
//! conversation as it was then (the task's words back in the prompt to change), or both.

use crate::compose::Rig;
use crate::picker::{Item, Terminal};
use crate::server::Ui;
use crate::server::asking::UiTerminal;
use protocol::Event;

/// Characters of a task shown in the list.
const SHOWN: usize = 70;

pub(crate) fn run(rig: &mut Rig, ui: &Ui) {
    let said = steps(rig, &mut UiTerminal { ui: ui.clone() });
    rig.output.note(&said);
}

fn steps(rig: &mut Rig, terminal: &mut dyn Terminal) -> String {
    let tasks = rig.checkpoints.tasks();
    if tasks.is_empty() {
        return "Nothing to rewind: no task yet in this conversation.\n".into();
    }
    // The newest first: the usual rewind is one step back.
    let items: Vec<Item> = tasks
        .iter()
        .rev()
        .map(|task| {
            let shown = crate::server::wire::shown(&task.goal);
            let line = shown.lines().next().unwrap_or("");
            let mut label: String = line.chars().take(SHOWN).collect();
            if label.len() < line.len() {
                label.push('…');
            }
            let files = match task.changed() {
                0 => "no file changed".to_string(),
                n => format!("{} changed", crate::output::count(n, "file")),
            };
            Item::new(label, files, false)
        })
        .collect();
    let Some(picked) = terminal.pick("Rewind to before", &items) else {
        return "Nothing changed.\n".into();
    };
    let index = tasks.len() - 1 - picked;
    let what = [
        Item::new(
            "Code and conversation",
            "the files and the talk as they were",
            true,
        ),
        Item::new("Conversation only", "the files stay as they are now", false),
        Item::new("Code only", "the conversation goes on as it is", false),
    ];
    let Some(how) = terminal.pick("Rewind", &what) else {
        return "Nothing changed.\n".into();
    };
    let mut said = vec![];
    match how {
        1 => rig.checkpoints.forget(index),
        _ => {
            let (restored, failed) = rig.checkpoints.restore(index);
            said.push(format!("{restored} file(s) put back"));
            said.extend(failed.into_iter().map(|f| format!("not put back: {f}")));
        }
    }
    if how != 2 {
        let task = &tasks[index];
        match &mut rig.conversation {
            Some(conversation) if task.at > 0 => conversation.truncate(task.at),
            _ => rig.conversation = None,
        }
        rig.agents.forget();
        said.push("the conversation is back to before it".into());
        // The person's own words go back in the prompt; what came from the background does not.
        if let Some(ui) = &rig.ui
            && crate::server::wire::shown(&task.goal) == task.goal
        {
            ui.send(Event::Prefill(task.goal.clone()));
        }
    }
    // The files are shared: another lane's task may have changed them since.
    let working = rig.shared.working.load(std::sync::atomic::Ordering::SeqCst);
    let warning = match working {
        0 => "",
        _ => "Another session is working in this directory; restoring may undo its changes.\n",
    };
    format!("{warning}Rewound: {}.\n", said.join("; "))
}

#[cfg(test)]
#[path = "rewind_tests.rs"]
mod tests;
