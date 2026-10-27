//! Where a client is, as the sessions it opens take it (docs/daemon.md, step 2): the workspace at
//! its directory, whose commands run with its environment (its `PATH`, a virtualenv, its keys,
//! less the app's hidden ones). In process the place is the process's own; over a socket,
//! each client's.

use std::path::Path;

use protocol::Place;
use tools::workspace::Workspace;

use crate::compose::APP;

/// The workspace at `dir`, with llx's config (keys, tokens) and sessions off limits to the tools:
/// a prompt injection must not read them.
pub(crate) fn at(dir: &Path) -> Result<Workspace, String> {
    let workspace = Workspace::new(dir).map_err(|error| error.to_string())?;
    if !workspace.root().is_dir() {
        return Err("not a directory".into());
    }
    let private = provider_kit::store::dir(APP).into_iter();
    let sessions = crate::commands::sessions::store().dir;
    Ok(workspace.denying(private.chain([sessions])))
}

/// The workspace a client's sessions work in: its directory, its environment.
pub(crate) fn workspace(place: &Place) -> Result<Workspace, String> {
    let workspace = at(Path::new(&place.cwd)).map_err(|why| format!("{}: {why}", place.cwd))?;
    Ok(workspace.with_env(place.env.clone()))
}

/// The variables that name a session's model, each with the option it stands for (its short
/// spelling too): read from the client's environment, never the daemon's.
pub(crate) const MODEL_VARS: [(&str, &str, Option<&str>); 4] = [
    ("LLX_PROVIDER", "--provider", None),
    ("LLX_MODEL", "--model", Some("-m")),
    ("LLX_REASONING", "--reasoning", None),
    ("LLX_DECISION_MODEL", "--decision-model", None),
];

/// What the client's environment says of its sessions' model that its command line does not
/// say itself, each by its variable: clap read the daemon's environment, not the client's.
pub(crate) fn chosen(place: &Place) -> Vec<(&'static str, String)> {
    let says = |long: &str, short: Option<&str>| {
        place.args.iter().any(|arg| {
            arg == long
                || arg.starts_with(&format!("{long}="))
                || short.is_some_and(|short| arg.starts_with(short))
        })
    };
    let mut chosen = vec![];
    for (name, long, short) in MODEL_VARS {
        let value = place.env.iter().find(|(var, _)| var == name);
        if let Some((_, value)) = value.filter(|(_, value)| !value.is_empty())
            && !says(long, short)
        {
            chosen.push((name, value.clone()));
        }
    }
    chosen
}

#[cfg(test)]
#[path = "place_tests.rs"]
mod tests;
