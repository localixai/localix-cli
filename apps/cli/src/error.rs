//! Why a command could not do its work, and the exit code each reason ends with. Commands return
//! a [`Failure`]; only `main` turns it into a message and an exit, so nothing below it exits.

use std::fmt;
use std::io;

use crate::exit;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The command line asks for something that cannot be: an empty prompt, a missing directory,
    /// a schema that is not JSON.
    Usage(String),
    /// What the command needs is not there: the network, a model, a file it must write.
    Environment(String),
    /// No OpenRouter key, from the environment or a sign-in.
    SignedOut,
    /// The reader of stdout went away (`lxsurf … | head -1`): nothing is left to tell anyone, so
    /// the command ends quietly, like any Unix filter.
    Closed,
}

impl Failure {
    /// The exit code this failure ends the process with (see [`exit`]).
    pub fn code(&self) -> i32 {
        match self {
            Self::Usage(_) | Self::Environment(_) | Self::SignedOut => exit::ENVIRONMENT,
            Self::Closed => exit::OK,
        }
    }

    /// What the person reads on stderr; `None` when nothing should be said.
    pub fn message(&self) -> Option<String> {
        match self {
            Self::Closed => None,
            other => Some(format!("lxsurf: {other}")),
        }
    }

    /// A failed write to stdout: a closed pipe ends quietly, anything else is the environment.
    pub fn from_write(error: &io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::BrokenPipe => Self::Closed,
            _ => Self::Environment(format!("could not write the output: {error}")),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) | Self::Environment(message) => f.write_str(message),
            Self::SignedOut => f.write_str(
                "not signed in: set OPENROUTER_API_KEY, or run `lxsurf login` \
                 (`lxsurf login --with-token` reads a key from stdin)",
            ),
            Self::Closed => f.write_str("the output was closed"),
        }
    }
}

impl std::error::Error for Failure {}

/// Write `text` to stdout and flush it: a closed pipe or another write error is a [`Failure`],
/// never a panic (`println!` panics on a closed pipe).
pub fn print(text: &str) -> Result<(), Failure> {
    use io::Write;
    let mut out = io::stdout().lock();
    out.write_all(text.as_bytes())
        .and_then(|()| out.flush())
        .map_err(|error| Failure::from_write(&error))
}

/// Write `text` to stderr, for the person; if even that fails, no one is left to tell.
pub fn note(text: &str) {
    use io::Write;
    let mut err = io::stderr();
    let _ = err.write_all(text.as_bytes()).and_then(|()| err.flush());
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
