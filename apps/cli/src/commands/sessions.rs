//! `lxsurf sessions` and `lxsurf resume`: every run is a session, kept as a JSON file of its tasks
//! and how each ended. The conversation itself is not kept, so `resume` starts a new one that is
//! told what the session asked before, and records into the same session.

use std::path::{Path, PathBuf};

use contracts::history::{History, Session};
use session::Sessions;

use crate::cli::ResumeArgs;
use crate::commands::run;
use crate::error::{self, Failure};
use crate::exit;
use crate::printable::printable;

/// Sessions listed at most.
const LISTED: usize = 30;
/// Characters of a session's first task in the list.
const TASK_CHARS: usize = 60;

/// Where sessions are kept: `LXSURF_SESSIONS`, else `~/.lxsurf/sessions`.
pub fn store() -> Sessions {
    let dir = match std::env::var_os("LXSURF_SESSIONS").filter(|dir| !dir.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => {
            let home = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .unwrap_or_default();
            PathBuf::from(home).join(".lxsurf/sessions")
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

/// One session as `lxsurf sessions` lists it: its id, when, how many tasks, where, the first one.
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

/// `lxsurf resume`: the session asked for, then its next task, or the prompt line. `--last` is
/// the newest session that ran in this workspace (else the newest of all, said so).
pub fn resume(args: ResumeArgs) -> Result<i32, Failure> {
    let (id, prompt) = args.target().map_err(Failure::Usage)?;
    let root = run::root(&args.run)?;
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
        None => store.load(id).map_err(|why| {
            Failure::Usage(format!("{why}: `lxsurf sessions` lists the saved ones"))
        })?,
    };
    if !session.workspace.is_empty() && session.workspace != here {
        error::note(&format!(
            "lxsurf: session {} ran in {}; this one runs in {here}\n",
            session.id,
            printable(&session.workspace)
        ));
    }
    let prompt = prompt.map(str::to_string);
    run::run(prompt.as_deref(), args.run, Some(session))
}

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
