//! What a run starts from when the command line does not say: the goal and whether the window is
//! visible.

use std::io::{IsTerminal, Read};

/// The goal from the command line, or from stdin for `-`.
pub fn read_goal(arg: Option<&str>) -> Result<String, String> {
    let goal = match arg {
        Some("-") => {
            let mut text = String::new();
            std::io::stdin()
                .read_to_string(&mut text)
                .map_err(|error| error.to_string())?;
            text
        }
        Some(goal) => goal.to_string(),
        None => return Err("no goal: decision-loop \"<what to do>\" [--url <page>]".into()),
    };
    let goal = goal.trim().to_string();
    if goal.is_empty() {
        return Err("the goal is empty".into());
    }
    Ok(goal)
}

/// Invisible by default under CI or outside a terminal (a test runner, a pipe); `--show` and
/// `--headless` override it.
pub fn headless_by_default() -> bool {
    let ci = std::env::var_os("CI").is_some_and(|value| !value.is_empty() && value != "false");
    ci || !std::io::stderr().is_terminal()
}
