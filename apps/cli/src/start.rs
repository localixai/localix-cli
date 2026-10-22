//! What a run starts from: the goal, the `--schema` it answers in, and whether the window is
//! visible when the command line does not say.

use std::io::{IsTerminal, Read};

use serde_json::Value;

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
        None => return Err("no goal: lxsurf \"<what to do>\"".into()),
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

/// `--schema`: inline JSON, or `@path` to a file.
pub fn read_schema(arg: &str) -> Result<Value, String> {
    let text = match arg.strip_prefix('@') {
        Some(path) => {
            std::fs::read_to_string(path).map_err(|error| format!("--schema {path}: {error}"))?
        }
        None => arg.to_string(),
    };
    let schema: Value =
        serde_json::from_str(&text).map_err(|error| format!("--schema is not JSON: {error}"))?;
    if !schema.is_object() {
        return Err("--schema must be a JSON Schema object".into());
    }
    Ok(schema)
}

#[cfg(test)]
#[path = "start_tests.rs"]
mod tests;
