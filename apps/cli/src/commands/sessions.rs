//! `lx sessions` and `lx resume`: every run is a session, kept as a JSON file of its tasks
//! and how each ended. The conversation itself is not kept, so `resume` starts a new one that is
//! told what the session asked before, and records into the same session.

use std::path::{Path, PathBuf};

use contracts::history::{History, Session};
use session::Sessions;

use crate::cli::RunOptions;
use crate::commands::run::{self, Kind};
use crate::error::{self, Failure};
use crate::exit;
use crate::printable::printable;

/// Sessions listed at most.
const LISTED: usize = 30;
/// Characters of a session's first task in the list.
const TASK_CHARS: usize = 60;

/// Where sessions are kept: `LX_SESSIONS`, else `~/.lx/sessions`.
pub fn store() -> Sessions {
    let dir = match std::env::var_os("LX_SESSIONS").filter(|dir| !dir.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => {
            let home = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .unwrap_or_default();
            PathBuf::from(home).join(".lx/sessions")
        }
    };
    Sessions { dir }
}

pub fn list() -> Result<i32, Failure> {
    let all = store().list();
    if all.is_empty() {
        error::print("No saved sessions.\n")?;
        return Ok(exit::OK);
    }
    let rows: String = all.iter().take(LISTED).map(row).collect();
    error::print(&rows)?;
    Ok(exit::OK)
}

/// One session as `lx sessions` lists it: its id, when, how many tasks, where, the first one.
pub fn row(session: &Session) -> String {
    let first = session.turns.first().map_or("", |turn| turn.goal.as_str());
    let first = printable(&first.split_whitespace().collect::<Vec<_>>().join(" "));
    let mut shown: String = first.chars().take(TASK_CHARS).collect();
    if shown.len() < first.len() {
        shown.push('…');
    }
    let place = match session.workspace.as_str() {
        "" => "?".to_string(),
        dir => printable(&home_relative(dir)),
    };
    let tasks = match session.turns.len() {
        1 => "1 task ".to_string(),
        n => format!("{n} tasks"),
    };
    format!(
        "{}  {:>10}  {tasks:>8}  {place}  {shown}\n",
        session.id,
        session::ago(session.updated)
    )
}

/// `dir` with the home directory written `~`, as a person recognizes it.
fn home_relative(dir: &str) -> String {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    let inside = home
        .as_deref()
        .map(Path::new)
        .filter(|home| !home.as_os_str().is_empty())
        .and_then(|home| Path::new(dir).strip_prefix(home).ok());
    match inside {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => Path::new("~").join(rest).display().to_string(),
        None => dir.to_string(),
    }
}

/// `lx resume` and `lx exec resume`: the session `id` names, else the newest one that ran in
/// this workspace (else the newest of all, said so), going on with `prompt` as `kind`.
pub fn resume(
    id: Option<&str>,
    prompt: Option<&str>,
    options: RunOptions,
    kind: Kind,
) -> Result<i32, Failure> {
    let root = run::root(&options)?;
    let here = root.root().display().to_string();
    let store = store();
    let newest_here = match id {
        Some(_) => None,
        None => store
            .list()
            .into_iter()
            .find(|session| session.workspace == here),
    };
    let session = match newest_here {
        Some(session) => session,
        None => store
            .load(id)
            .map_err(|why| Failure::Usage(format!("{why}: `lx sessions` lists the saved ones")))?,
    };
    if !session.workspace.is_empty() && session.workspace != here {
        error::note(&format!(
            "lx: session {} ran in {}; this one runs in {here}\n",
            session.id,
            printable(&session.workspace)
        ));
    }
    run::run(prompt, options, Some(session), kind)
}

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
