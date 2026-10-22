//! What the agent knows of where it runs before its first call: the working directory, the
//! system, the date, the mode it works in, and the project's own instructions (its AGENTS.md
//! files), found as Codex finds them: from the repository's root down to the working directory,
//! so a deeper file comes later and wins.

use std::path::{Path, PathBuf};

use contracts::history::Session;

/// The project instruction file read in each directory.
const INSTRUCTIONS: &str = "AGENTS.md";
/// Bytes of one instruction file: a huge one must not crowd out the rest.
const FILE_LIMIT: usize = 16 * 1024;
/// Bytes of all instruction files together.
const TOTAL_LIMIT: usize = 32 * 1024;
/// Bytes of an earlier session's answers a resumed session is told.
const PREVIOUS_LIMIT: usize = 8 * 1024;
/// Bytes of one earlier answer.
const ANSWER_LIMIT: usize = 1500;
/// Bytes of one earlier task: a goal may carry a whole piped file in its `<stdin>` block.
const GOAL_LIMIT: usize = 500;

/// Where the run happens, as the context block's first part says it.
pub struct Environment<'a> {
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
pub fn gather(environment: &Environment, previous: Option<&Session>) -> String {
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

/// The AGENTS.md files from the repository root down to `root`, each in its own block, capped.
pub fn instructions(root: &Path) -> String {
    let mut left = TOTAL_LIMIT;
    let mut blocks = vec![];
    for dir in chain(root) {
        let path = dir.join(INSTRUCTIONS);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let text = text.trim();
        if text.is_empty() || left == 0 {
            continue;
        }
        let kept = cut(text, FILE_LIMIT.min(left));
        left -= kept.len();
        let more = match text.len() - kept.len() {
            0 => String::new(),
            cut => format!("\n[{cut} more bytes not shown: read the file for the rest]"),
        };
        blocks.push(format!(
            "<project_instructions path=\"{}\">\n{kept}{more}\n</project_instructions>",
            path.display()
        ));
    }
    blocks.join("\n\n")
}

/// The directories whose instructions apply to `root`: from the repository's root (the nearest
/// ancestor with `.git`) down to `root`; only `root` outside a repository.
fn chain(root: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![];
    for dir in root.ancestors() {
        dirs.push(dir.to_path_buf());
        if dir.join(".git").exists() {
            dirs.reverse();
            return dirs;
        }
    }
    vec![root.to_path_buf()]
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
fn cut(text: &str, limit: usize) -> &str {
    if text.len() <= limit {
        return text;
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// `2026-10-22` for Unix seconds, in UTC (the civil-from-days algorithm: no date crate needed).
pub fn date(unix: u64) -> String {
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
