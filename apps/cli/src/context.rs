//! What the agent knows of where it runs before its first call: the working directory, the
//! system, the date, the mode it works in, and the instructions it follows (see
//! [`crate::instructions`]). It is the same for every task and session in a workspace on a day,
//! so the prompt cache keeps it: nothing in it changes by the minute.

use std::path::{Path, PathBuf};

use contracts::history::Session;

use crate::instructions;

/// Bytes of an earlier session's answers a resumed session is told.
const PREVIOUS_LIMIT: usize = 8 * 1024;
/// Bytes of one earlier answer.
const ANSWER_LIMIT: usize = 1500;
/// Bytes of one earlier task: a goal may carry a whole piped file in its `<stdin>` block.
const GOAL_LIMIT: usize = 500;

/// Where the run happens, as the context block's first part says it.
pub(crate) struct Environment<'a> {
    /// The workspace root (canonical).
    pub root: &'a Path,
    /// The mode: what runs without asking, what is refused, whether questions are answered.
    pub mode: &'a str,
    /// Whether the browser tool is offered.
    pub browser: bool,
    /// Unix seconds now.
    pub now: u64,
}

/// The whole context: the environment, the project's instructions, and what a resumed session
/// did before.
pub(crate) fn gather(environment: &Environment, previous: Option<&Session>) -> String {
    let mut parts = vec![describe(environment)];
    let instructions = instructions(environment.root);
    if !instructions.is_empty() {
        parts.push(instructions);
    }
    if let Some(session) = previous.filter(|session| !session.turns.is_empty()) {
        parts.push(earlier(session, environment.root));
    }
    parts.join("\n\n")
}

/// The context `fresh` (gathered with no earlier session) going on from `session`: a saved
/// session opened in a lane of its own reads as if the run had resumed it.
pub(crate) fn resumed(fresh: &str, session: &Session, root: &Path) -> String {
    match session.turns.is_empty() {
        true => fresh.to_string(),
        false => format!("{fresh}\n\n{}", earlier(session, root)),
    }
}

/// The shell is not said here: the `bash` tool's own description names it.
fn describe(environment: &Environment) -> String {
    let browser = if environment.browser {
        "available (a window opens on its first use)"
    } else {
        "not available here (no display)"
    };
    format!(
        "<environment>\nWorking directory: {} (the workspace root: files change only inside it)\n\
         System: {} {}\nDate: {} (UTC)\nMode: {}\nBrowser: {browser}\n\
         </environment>",
        environment.root.display(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        date(environment.now),
        environment.mode,
    )
}

/// The person's own instructions, then the project's from the repository root down to `root`,
/// each in its own block, capped.
pub(crate) fn instructions(root: &Path) -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut blocks = vec![];
    if let Some(own) = instructions::personal(home.as_deref()) {
        blocks.push(format!(
            "<personal_instructions path=\"{}\">\n{}\n</personal_instructions>",
            own.path.display(),
            own.text
        ));
    }
    for found in instructions::project(root, instructions::TOTAL_LIMIT) {
        blocks.push(instructions::block(&found, None));
    }
    blocks.join("\n\n")
}

/// What an earlier session asked and how each task ended, newest kept when it is long, and where
/// it ran when that is not `root`.
fn earlier(session: &Session, root: &Path) -> String {
    let mut left = PREVIOUS_LIMIT;
    let mut turns = vec![];
    for (number, turn) in session.turns.iter().enumerate().rev() {
        let answer = cut(turn.answer.trim(), ANSWER_LIMIT);
        let entry = format!(
            "{}. Task: {}\n   Ended: {}\n   Answer: {answer}",
            number + 1,
            goal(&turn.goal),
            turn.status
        );
        // Each entry is capped, so an older one may still fit after a newer did not.
        if entry.len() > left {
            continue;
        }
        left -= entry.len();
        turns.push(entry);
    }
    turns.reverse();
    let here = root.display().to_string();
    let place = match session.workspace.as_str() {
        "" => String::new(),
        dir if dir == here => format!("Its tasks ran in this workspace, {dir}.\n"),
        dir => format!(
            "Its tasks ran in {dir}, not in this workspace: what they changed is there, not here.\n"
        ),
    };
    let mut out = format!(
        "<previous_session id=\"{}\">\nThis continues an earlier session. Its conversation was \
         not kept, only each task and how it ended; check the current state with the tools.\n\
         {place}{}",
        session.id,
        turns.join("\n")
    );
    if !session.location.is_empty() {
        out.push_str(&format!("\nThe browser was last at: {}", session.location));
    }
    out.push_str("\n</previous_session>");
    out
}

/// An earlier task as the context tells it: its `<stdin>` block left out, and capped.
fn goal(goal: &str) -> String {
    let asked = match goal.split_once("\n\n<stdin>\n") {
        Some((asked, _)) => format!("{} [with stdin, not kept here]", asked.trim()),
        None => goal.trim().to_string(),
    };
    match cut(&asked, GOAL_LIMIT) {
        kept if kept.len() < asked.len() => format!("{kept}…"),
        kept => kept.to_string(),
    }
}

/// At most `limit` bytes of `text`, cut on a character boundary.
pub(crate) fn cut(text: &str, limit: usize) -> &str {
    let end = (0..=limit.min(text.len()))
        .rev()
        .find(|at| text.is_char_boundary(*at));
    text.get(..end.unwrap_or(0)).unwrap_or_default()
}

/// `2026-10-22` for Unix seconds, in UTC (the civil-from-days algorithm: no date crate needed).
pub(crate) fn date(unix: u64) -> String {
    let days = (unix / 86_400) as i64 + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
