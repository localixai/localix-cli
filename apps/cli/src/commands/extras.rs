//! The small commands: `/diff` (the working tree's changes, new
//! files too), `/init` (an AGENTS.md written for the project), `/copy` (the last answer to the
//! clipboard) and `/usage` (what the session spent).

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::compose::Rig;

/// Rows of a diff shown at most; the rest is counted.
const DIFF_ROWS: usize = 400;

/// What `/init` asks.
pub(crate) const INIT: &str = "Write an AGENTS.md at the root of this repository for coding agents \
working in it: what the project is, how it is laid out (the main directories and what lives \
where), how to build, test, lint and run it (the exact commands, checked by running them where \
that is quick and harmless), the conventions the code follows (style, naming, tests, commits), \
and anything a newcomer would get wrong. Read before you write: the README, the manifests, the \
CI configuration, a few representative files. Keep it short and concrete, every line something \
an agent will act on. If an AGENTS.md or CLAUDE.md exists, improve it instead of replacing it.";

/// `/diff`: what changed since the last commit, and the files git does not track yet.
#[allow(
    clippy::disallowed_methods,
    reason = "R10 debt: a local git query, with no deadline or stop yet"
)]
pub(crate) fn diff(root: &Path) -> String {
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).to_string())
    };
    let Some(changed) = git(&["diff", "HEAD", "--no-color"]).or_else(|| git(&["diff"])) else {
        return "Not a git repository here: nothing to compare.\n".into();
    };
    let untracked = git(&["ls-files", "--others", "--exclude-standard"]).unwrap_or_default();
    let mut rows: Vec<String> = changed.lines().map(str::to_string).collect();
    rows.extend(
        untracked
            .lines()
            .map(|path| format!("new file (untracked): {path}")),
    );
    if rows.is_empty() {
        return "No changes since the last commit.\n".into();
    }
    let more = rows.len().saturating_sub(DIFF_ROWS);
    rows.truncate(DIFF_ROWS);
    if more > 0 {
        rows.push(format!("… {more} more rows: run git diff to see them all"));
    }
    rows.join("\n") + "\n"
}

/// `/copy`: the last answer to the system clipboard.
pub(crate) fn copy(rig: &Rig) -> String {
    let Some(answer) = last_answer(&rig.session.turns) else {
        return "No answer to copy yet.\n".into();
    };
    #[cfg(target_os = "macos")]
    let tools: &[&[&str]] = &[&["pbcopy"]];
    #[cfg(target_os = "windows")]
    let tools: &[&[&str]] = &[&["clip"]];
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let tools: &[&[&str]] = &[
        &["wl-copy"],
        &["xclip", "-selection", "clipboard"],
        &["xsel", "-ib"],
    ];
    for tool in tools {
        let spawned = Command::new(tool[0])
            .args(&tool[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = spawned else {
            continue;
        };
        let written = child
            .stdin
            .take()
            .map(|mut stdin| stdin.write_all(answer.as_bytes()));
        if matches!(written, Some(Ok(()))) && child.wait().is_ok_and(|s| s.success()) {
            return format!(
                "Copied the last answer ({} characters).\n",
                answer.chars().count()
            );
        }
    }
    "No clipboard tool here (pbcopy, wl-copy, xclip, xsel or clip).\n".into()
}

/// The last answer that says something: a failed or stopped task leaves none, and copying
/// nothing would empty the clipboard.
fn last_answer(turns: &[contracts::history::Turn]) -> Option<String> {
    let mut answers = turns.iter().rev().map(|turn| turn.answer.trim());
    answers
        .find(|answer| !answer.is_empty())
        .map(str::to_string)
}

/// `/usage`: what the session's tasks spent, the models' turns and tokens and the time.
pub(crate) fn usage(rig: &Rig) -> String {
    use crate::output::human::{thousands, tokens_in};
    let spent = &rig.spent;
    let decision = rig.meter.spend();
    let minutes = rig.begun.elapsed().as_secs() / 60;
    let session = rig.output.total();
    format!(
        "tasks      {}\nagent      {} · {} turns · {} · {} out · ${:.4}\n\
         decision   {} calls · ${:.4}\ntotal      ${:.4} over {minutes} min\n\
         session    {} · {} out · ${:.4}\n",
        rig.session.turns.len(),
        rig.choice.model(),
        spent.calls,
        tokens_in(spent.input_tokens, spent.cached_tokens),
        thousands(spent.output_tokens),
        spent.cost,
        decision.calls,
        decision.cost,
        spent.cost + decision.cost,
        tokens_in(session.input_tokens, session.cached_tokens),
        thousands(session.output_tokens),
        session.cost,
    )
}

#[cfg(test)]
#[path = "extras_tests.rs"]
mod tests;
