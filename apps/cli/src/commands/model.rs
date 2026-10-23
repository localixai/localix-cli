//! `/model` and the reasoning level: one of the models of the providers signed in to, then a
//! level it takes; Shift+Up and Shift+Down step the level. A model switch starts at that model's
//! default level, and only its own levels are offered. The choice is put in at once (the chat
//! model, and its key's variables kept from commands) and saved as the default for later
//! sessions. `/login` ends with the same model step.

use std::path::PathBuf;

use contracts::catalog::{ModelEntry, Selection};

use crate::choice::{self, Choice};
use crate::compose::Rig;
use crate::picker::{Item, Terminal};
use crate::providers::{self, Provider};
use crate::tui::Ui;
use crate::tui::asking::UiTerminal;

/// A level the model takes; asked only when it takes more than one.
fn reasoning(selection: Selection, terminal: &mut dyn Terminal) -> Option<Selection> {
    let offered = selection.model().offered().to_vec();
    if offered.len() == 1 {
        return Some(selection);
    }
    let default = selection.model().default;
    let items: Vec<Item> = offered
        .iter()
        .map(|&effort| {
            let note = if effort == default { " (default)" } else { "" };
            let detail = format!("{}{note}", effort.describe());
            Item::new(effort.name(), detail, effort == selection.effort())
        })
        .collect();
    let at = terminal.pick(&format!("Reasoning ({})", selection.model().name), &items)?;
    selection.with_effort(offered[at])
}

/// `/model`: one of the models of every provider signed in to (the current one marked), then a
/// level it takes; `None` when the person cancelled, or nobody is signed in.
pub fn pick_model(
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

/// One of the models of `providers` under `title`, then a level it takes; `None` when the person
/// cancelled.
pub(super) fn models(
    current: &Choice,
    terminal: &mut dyn Terminal,
    providers: &[Provider],
    title: &str,
) -> Option<Choice> {
    let mut offered: Vec<(Provider, ModelEntry)> = vec![];
    for provider in providers {
        let mut models = provider.models();
        if *provider == current.provider && !models.iter().any(|m| m.id == current.model()) {
            models.insert(0, current.selection.model().clone());
        }
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
        .map(|(at, (provider, model))| {
            let levels = choice::levels(&Selection::new(model.clone()));
            let detail = format!("{} · {}  reasoning: {levels}", provider.name(), model.id);
            Item::new(&model.name, detail, Some(at) == marked)
        })
        .collect();
    let at = terminal.pick(title, &items)?;
    let kept = now(&offered[at]);
    let (provider, model) = offered.swap_remove(at);
    // The same model keeps its level; another one starts at its own default.
    let selection = match kept {
        true => current.selection.clone(),
        false => Selection::new(model),
    };
    reasoning(selection, terminal).map(|selection| Choice {
        provider,
        selection,
    })
}

/// `/model`: the pick, then the choice put in and saved.
pub fn run_model(rig: &mut Rig, ui: &Ui) {
    let terminal = &mut UiTerminal { ui: ui.clone() };
    match pick_model(&rig.choice, terminal, &Provider::signed_in) {
        Some(choice) => rig.output.note(&now(rig, choice)),
        None => rig.output.note("Nothing changed.\n"),
    }
}
/// Shift+Up or Shift+Down: the model's next level up or down, put in and saved.
pub fn step_reasoning(rig: &mut Rig, up: bool) -> String {
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
    let Some(chat) = choice.provider.chat(&choice.selection) else {
        let name = choice.provider.name();
        return Err(format!("Not signed in to {name}: nothing changed.\n"));
    };
    rig.hide_for(&choice.provider);
    if let Some(live) = rig.live {
        live.chat.set(chat);
        live.decider
            .set(providers::decider(&rig.options.models.decision_model));
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
