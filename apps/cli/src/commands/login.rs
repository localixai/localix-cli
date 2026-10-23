//! `/login` in a session, as a few steps, each a pick: the provider (every one of the catalog,
//! searched by typing, with how it is signed in to), signing in when it is not (OAuth in the
//! browser, or a pasted key: see `signin`), one of its models, and a reasoning level that model
//! takes (see `model`). The choice is put in at once and saved as the default for later sessions.

use crate::choice::Choice;
use crate::commands::model::{models, now};
use crate::commands::signin;
use crate::compose::Rig;
use crate::picker::{Item, Terminal};
use crate::providers::{Method, Provider};
use crate::server::Ui;
use crate::server::asking::UiTerminal;
use protocol::Event;

/// What the steps ask of the world beyond the terminal: where each provider's sign-in comes
/// from, and signing in (what was saved, in words).
pub(crate) struct World<'a> {
    pub signed_in: &'a dyn Fn(&Provider) -> Option<String>,
    pub sign_in: &'a SignIn<'a>,
    pub say: &'a dyn Fn(&str),
}

/// Sign in to a provider in a way, asking at the terminal; what was saved, in words.
pub(crate) type SignIn<'a> =
    dyn Fn(Provider, Method, &mut dyn Terminal) -> Result<String, String> + 'a;

/// The steps from `current`: the choice made, `Ok(None)` when the person cancelled.
pub(crate) fn choose(
    current: &Choice,
    terminal: &mut dyn Terminal,
    world: &World,
) -> Result<Option<Choice>, String> {
    let mut offered = offered(Provider::all(), &current.provider, world.signed_in);
    let items: Vec<Item> = offered
        .iter()
        .map(|provider| {
            let id = provider.id();
            let (mark, detail) = match ((world.signed_in)(provider), provider.hint()) {
                (Some(from), _) => (" ✓", format!("{id} · signed in: {from}")),
                (None, Some(hint)) => ("", format!("{id} · not signed in · {hint}")),
                (None, None) => ("", format!("{id} · not signed in")),
            };
            let label = format!("{}{mark}", provider.name());
            Item::new(label, detail, *provider == current.provider)
        })
        .collect();
    let Some(at) = terminal.pick("Provider", &items) else {
        return Ok(None);
    };
    let provider = offered.swap_remove(at);
    if !signed_in(&provider, terminal, world)? {
        return Ok(None);
    }
    let title = format!("Model ({})", provider.name());
    Ok(models(current, terminal, &[provider], &title))
}

/// Every provider of the catalog, in the order a person looks for one: the popular (in their
/// order), then the current one and those signed in to, then the rest, each by name.
fn offered(
    mut all: Vec<Provider>,
    current: &Provider,
    signed_in: &dyn Fn(&Provider) -> Option<String>,
) -> Vec<Provider> {
    all.sort_by_cached_key(
        |p| match models::POPULAR.iter().position(|id| *id == p.id()) {
            Some(at) => (0, at, String::new()),
            None if p == current || signed_in(p).is_some() => (1, 0, p.name().to_lowercase()),
            None => (2, 0, p.name().to_lowercase()),
        },
    );
    all
}

/// Keep the sign-in there is, or sign in; `false` when the person cancelled.
fn signed_in(
    provider: &Provider,
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
    let said = (world.sign_in)(provider.clone(), method, terminal)?;
    (world.say)(&format!("{said}\n"));
    Ok(true)
}

fn describe(method: Method) -> &'static str {
    match method {
        Method::Key => "Paste an API key",
        Method::Browser => "Sign in in the browser",
    }
}

/// `/login`: the steps, then the choice put in and saved.
pub(crate) fn run(rig: &mut Rig, ui: &Ui) {
    let say = |text: &str| ui.send(Event::Note(text.to_string()));
    steps(rig, &mut UiTerminal { ui: ui.clone() }, &say);
}

fn steps(rig: &mut Rig, terminal: &mut dyn Terminal, shown: &dyn Fn(&str)) {
    let sign_in = |provider, method, terminal: &mut dyn Terminal| {
        signin::sign_in(provider, method, terminal, shown)
    };
    let world = World {
        signed_in: &Provider::signed_in,
        sign_in: &sign_in,
        say: shown,
    };
    match choose(&rig.choice, terminal, &world) {
        Ok(Some(choice)) => rig.output.note(&now(rig, choice)),
        Ok(None) => rig.output.note("Nothing changed.\n"),
        Err(why) => rig.output.note(&format!("Not signed in: {why}\n")),
    }
}

/// How the session stands with its provider, for `/status`.
pub(crate) fn signed(choice: &Choice) -> String {
    match choice.provider.signed_in() {
        Some(from) => format!("signed in: {from}"),
        None => format!("not signed in to {}: type /login", choice.provider.name()),
    }
}

/// A line for the banner when the session is signed out, else nothing.
pub(crate) fn signed_out(choice: &Choice) -> String {
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
