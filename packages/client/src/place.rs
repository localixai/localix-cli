//! Where an interface is, as it tells the server at its handshake: both interfaces (the
//! terminal's and the tool clients') read it the same way from their own process.

use std::path::Path;

use protocol::{Place, Theme};

/// This process's place: its `cwd` (where the sessions work), its environment (a variable that
/// is not text is left out), its working directory (`dir`, which the relative paths of `args`
/// are of), the command line `args` it is to be served as, and its terminal's colours.
#[must_use]
pub fn here(cwd: &Path, theme: Option<Theme>, args: Vec<String>) -> Place {
    let env = std::env::vars_os()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)));
    let dir = std::env::current_dir().unwrap_or_default();
    Place {
        cwd: cwd.display().to_string(),
        env: env.collect(),
        theme,
        args,
        dir: dir.display().to_string(),
    }
}

#[cfg(test)]
#[path = "place_tests.rs"]
mod tests;
