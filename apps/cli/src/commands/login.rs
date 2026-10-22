//! `/login` in a session, as a few steps, each a pick: the provider (with how it is signed in
//! to), signing in when it is not (OAuth in the browser, or a pasted key: see `signin`), one of
//! its models, and a reasoning level that model takes. A model switch starts at that model's
//! default level, and only its own levels are offered. The choice is put in at once and saved as
//! the default for later sessions.

use std::io::BufRead;

use contracts::catalog::Selection;

use crate::choice::{self, Choice};
use crate::commands::signin;
use crate::compose::Rig;
use crate::picker::{Item, Numbered, Terminal};
use crate::providers::{self, Method, Provider};

/// What the steps ask of the world beyond the terminal: where each provider's sign-in comes
/// from, and signing in (what was saved, in words).
pub struct World<'a> {
    pub signed_in: &'a dyn Fn(Provider) -> Option<String>,
    pub sign_in: &'a SignIn<'a>,
    pub say: &'a dyn Fn(&str),
}

/// Sign in to a provider in a way, asking at the terminal; what was saved, in words.
pub type SignIn<'a> = dyn Fn(Provider, Method, &mut dyn Terminal) -> Result<String, String> + 'a;

/// The steps from `current`: the choice made, `Ok(None)` when the person cancelled.
pub fn choose(
    current: &Choice,
    terminal: &mut dyn Terminal,
    world: &World,
) -> Result<Option<Choice>, String> {
    let items: Vec<Item> = Provider::ALL
        .iter()
        .map(|&provider| {
            let detail = (world.signed_in)(provider)
                .map_or("not signed in".into(), |from| format!("signed in: {from}"));
            Item::new(provider.name(), detail, provider == current.provider)
        })
        .collect();
    let Some(at) = terminal.pick("Provider", &items) else {
        return Ok(None);
    };
    let provider = Provider::ALL[at];
    if !signed_in(provider, terminal, world)? {
        return Ok(None);
    }
    let Some(model) = model(provider, current, terminal) else {
        return Ok(None);
    };
    Ok(reasoning(model, terminal).map(|selection| Choice {
        provider,
        selection,
    }))
}

/// Keep the sign-in there is, or sign in; `false` when the person cancelled.
fn signed_in(
    provider: Provider,
    terminal: &mut dyn Terminal,
    world: &World,
) -> Result<bool, String> {
    if let Some(from) = (world.signed_in)(provider) {
        let items = [
            Item::new("Keep it", "", true),
            Item::new("Sign in again", "", false),
        ];
        let title = format!("Signed in to {} with {from}", provider.name());
        match terminal.pick(&title, &items) {
            None => return Ok(false),
            Some(0) => return Ok(true),
            Some(_) => {}
        }
    }
    let methods = provider.methods();
    let method = match methods {
        [only] => *only,
        _ => {
            let items: Vec<Item> = methods
                .iter()
                .enumerate()
                .map(|(at, method)| Item::new(describe(*method), "", at == 0))
                .collect();
            let Some(at) = terminal.pick("Sign in with", &items) else {
                return Ok(false);
            };
            methods[at]
        }
    };
    let said = (world.sign_in)(provider, method, terminal)?;
    (world.say)(&format!("{said}\n"));
    Ok(true)
}

fn describe(method: Method) -> &'static str {
    match method {
        Method::Key => "Paste an API key",
        Method::Browser => "Sign in in the browser",
    }
}

/// One of the provider's models: the current one marked, else the provider's default.
fn model(provider: Provider, current: &Choice, terminal: &mut dyn Terminal) -> Option<Selection> {
    let mut models = provider.models();
    let same = current.provider == provider;
    if same && !models.iter().any(|model| model.id == current.model()) {
        models.insert(0, current.selection.model().clone());
    }
    let marked = if same {
        current.model()
    } else {
        provider.default_model()
    };
    let items: Vec<Item> = models
        .iter()
        .map(|model| {
            let levels = choice::levels(&Selection::new(model.clone()));
            let detail = format!("{}  reasoning: {levels}", model.id);
            Item::new(&model.name, detail, model.id == marked)
        })
        .collect();
    let at = terminal.pick(&format!("Model ({})", provider.name()), &items)?;
    let model = models.swap_remove(at);
    // The same model keeps its level; another one starts at its own default.
    Some(match same && model.id == current.model() {
        true => current.selection.clone(),
        false => Selection::new(model),
    })
}

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

/// `/login` at the prompt line: the steps, then the choice put in and saved.
pub fn run(rig: &mut Rig, input: &mut dyn BufRead, shown: &dyn Fn(&str)) {
    let mut terminal = Numbered { input, shown };
    let sign_in = |provider, method, terminal: &mut dyn Terminal| {
        signin::sign_in(provider, method, terminal, shown)
    };
    let world = World {
        signed_in: &Provider::signed_in,
        sign_in: &sign_in,
        say: shown,
    };
    match choose(&rig.choice, &mut terminal, &world) {
        Ok(Some(choice)) => rig.output.note(&adopt(rig, choice)),
        Ok(None) => rig.output.note("Nothing changed.\n"),
        Err(why) => rig.output.note(&format!("Not signed in: {why}\n")),
    }
}

/// Think with `choice` from the next task on, and save it as the default.
fn adopt(rig: &mut Rig, choice: Choice) -> String {
    let Some(chat) = choice.provider.chat(&choice.selection) else {
        return format!(
            "Not signed in to {}: nothing changed.\n",
            choice.provider.name()
        );
    };
    if let Some(live) = rig.live {
        live.chat.set(chat);
        live.decider
            .set(providers::decider(&rig.options.models.decision_model));
    }
    let saved = match choice::save(&choice) {
        Ok(path) => format!("saved as the default in {}", path.display()),
        Err(error) => format!("not saved: {error}"),
    };
    rig.choice = choice;
    format!("Now {} ({saved}).\n", rig.choice.label())
}

/// How the session stands with its provider, for `/status`.
pub fn signed(choice: &Choice) -> String {
    match choice.provider.signed_in() {
        Some(from) => format!("signed in: {from}"),
        None => format!("not signed in to {}: type /login", choice.provider.name()),
    }
}

/// A line for the banner when the session is signed out, else nothing.
pub fn signed_out(choice: &Choice) -> String {
    match choice.provider.signed_in() {
        Some(_) => String::new(),
        None => format!(
            "Not signed in to {}: type /login.\n",
            choice.provider.name()
        ),
    }
}

#[cfg(test)]
#[path = "login_tests.rs"]
mod tests;
