//! `/model` and the reasoning level: one list, each provider's models a group of their own (a
//! tab in the interface), with Left/Right choosing the level inline.
//! Shift+Up and Shift+Down also step the level. A model switch starts at that model's
//! default level, and only its own levels are offered. The choice is put in at once (the chat
//! model, and its key's variables kept from commands) and saved as the default for later
//! sessions. `/login` ends with the same model step.

use std::path::PathBuf;

use contracts::catalog::{ModelEntry, Selection};

use crate::choice::{self, Choice};
use crate::compose::Rig;
use crate::picker::{Adjustment, Group, Item, Terminal};
use crate::providers::{self, Provider};
use crate::server::Ui;
use crate::server::asking::UiTerminal;

/// `/model`: a signed-in provider's model and its level in one list; `None` on cancellation
/// or when nobody is signed in.
pub(crate) fn pick_model(
    current: &Choice,
    terminal: &mut dyn Terminal,
    signed_in: &dyn Fn(&Provider) -> Option<String>,
) -> Option<Choice> {
    let providers: Vec<Provider> = Provider::all()
        .into_iter()
        .filter(|p| *p == current.provider || signed_in(p).is_some())
        .collect();
    models(current, terminal, &providers, "Model")
}

/// A model of `providers` and its inline level under `title`; `None` when the person cancels.
pub(super) fn models(
    current: &Choice,
    terminal: &mut dyn Terminal,
    providers: &[Provider],
    title: &str,
) -> Option<Choice> {
    let mut offered: Vec<(Provider, ModelEntry)> = vec![];
    // Each provider's models together, under its name: the interface shows them as its tab.
    let mut groups: Vec<Group> = vec![];
    for provider in providers {
        let mut models = provider.models();
        if *provider == current.provider && !models.iter().any(|m| m.id == current.model()) {
            models.insert(0, current.selection.model().clone());
        }
        let rows = models.len();
        groups.push(Group {
            name: provider.name(),
            rows,
        });
        offered.extend(models.into_iter().map(|model| (provider.clone(), model)));
    }
    let now = |(provider, model): &(Provider, ModelEntry)| {
        *provider == current.provider && model.id == current.model()
    };
    // The current model, else the first provider's default.
    let default = providers.first().map(Provider::default_model);
    let marked = offered.iter().position(now).or_else(|| {
        offered
            .iter()
            .position(|(_, m)| Some(&m.id) == default.as_ref())
    });
    let items: Vec<Item> = offered
        .iter()
        .enumerate()
        .map(|(at, (_, model))| Item::new(&model.name, &model.id, Some(at) == marked))
        .collect();
    let adjustments: Vec<Adjustment> = offered
        .iter()
        .map(|entry| {
            let model = &entry.1;
            // The same model keeps its level; another starts at its own default.
            let effort = if now(entry) {
                current.selection.effort()
            } else {
                model.default
            };
            let levels = model.offered();
            Adjustment {
                values: levels.iter().map(|e| e.name().to_string()).collect(),
                at: levels.iter().position(|e| *e == effort).unwrap_or(0),
            }
        })
        .collect();
    let (at, level) = terminal.pick_adjustable(title, &items, &adjustments, &groups)?;
    let (provider, model) = offered.swap_remove(at);
    let effort = *model.offered().get(level)?;
    Some(Choice {
        provider,
        selection: Selection::new(model).with_effort(effort)?,
    })
}

/// `/model`: the pick, then the choice put in and saved.
pub(crate) fn run_model(rig: &mut Rig, ui: &Ui) {
    let terminal = &mut UiTerminal { ui: ui.clone() };
    match pick_model(&rig.choice, terminal, &Provider::signed_in) {
        Some(choice) => rig.output.note(&now(rig, choice)),
        None => rig.output.note("Nothing changed.\n"),
    }
}
/// Shift+Up or Shift+Down: the model's next level up or down, put in and saved.
pub(crate) fn step_reasoning(rig: &mut Rig, up: bool) -> String {
    let selection = match stepped(&rig.choice.selection, up) {
        Ok(selection) => selection,
        Err(why) => return why,
    };
    let choice = Choice {
        selection,
        ..rig.choice.clone()
    };
    // A step is pressed again and again: a short note, the file's path is /status's to say.
    match adopt(rig, choice) {
        Ok(saved) => {
            let saved = saved.map_or_else(|e| format!("not saved: {e}"), |_| "saved".into());
            format!(
                "Reasoning: {} ({saved}).\n",
                rig.choice.selection.effort().name()
            )
        }
        Err(why) => why,
    }
}

/// `selection` a level up or down; what to say when there is none.
fn stepped(selection: &Selection, up: bool) -> Result<Selection, String> {
    let model = &selection.model().id;
    if selection.model().efforts.is_empty() {
        return Err(format!("{model} takes no reasoning levels.\n"));
    }
    let offered = selection.model().offered();
    let now = offered.iter().position(|e| *e == selection.effort());
    let next = match (now, up) {
        (Some(at), true) => offered.get(at + 1),
        (Some(at), false) => at.checked_sub(1).and_then(|at| offered.get(at)),
        (None, _) => offered.first(),
    };
    next.and_then(|e| selection.with_effort(*e)).ok_or_else(|| {
        let end = if up { "the most" } else { "the least" };
        format!("{model} takes no more: this is {end} it reasons.\n")
    })
}

/// Think with `choice` from the next task on, and save it as the default: where it was saved,
/// or why not; `Err` with what to say when its provider is not signed in.
fn adopt(rig: &mut Rig, choice: Choice) -> Result<std::io::Result<PathBuf>, String> {
    let vars = rig.workspace.env();
    let Some(chat) = choice.provider.chat_in(&choice.selection, vars) else {
        let name = choice.provider.name();
        return Err(format!("Not signed in to {name}: nothing changed.\n"));
    };
    rig.hide_for(&choice.provider);
    if let Some(live) = rig.live {
        live.chat.set(chat);
        live.decider.set(providers::decider_in(
            &rig.options.models.decision_model,
            vars,
        ));
    }
    let saved = choice::save(&choice);
    rig.choice = choice;
    Ok(saved)
}

/// [`adopt`], said in full.
pub(super) fn now(rig: &mut Rig, choice: Choice) -> String {
    let saved = match adopt(rig, choice) {
        Ok(Ok(path)) => format!("saved as the default in {}", path.display()),
        Ok(Err(error)) => format!("not saved: {error}"),
        Err(why) => return why,
    };
    format!("Now {} ({saved}).\n", rig.choice.label())
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
