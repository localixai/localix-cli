//! Why a command could not do its work, and the exit code each reason ends with. Commands return
//! a [`Failure`]; only `main` turns it into a message and an exit, so nothing below it exits.

use std::fmt;
use std::io;

use crate::exit;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Failure {
    /// The command line asks for something that cannot be: an empty prompt, a missing directory,
    /// a schema that is not JSON.
    Usage(String),
    /// What the command needs is not there: the network, a model, a file it must write.
    Environment(String),
    /// No sign-in to the chosen provider (a key, or ChatGPT tokens): how to sign in to it.
    SignedOut(String),
    /// The reader of stdout went away (`llx … | head -1`): nothing is left to tell anyone, so
    /// the command ends quietly, like any Unix filter.
    Closed,
}

impl Failure {
    /// The exit code this failure ends the process with (see [`exit`]).
    pub(crate) fn code(&self) -> i32 {
        match self {
            Self::Usage(_) | Self::Environment(_) | Self::SignedOut(_) => exit::ENVIRONMENT,
            Self::Closed => exit::OK,
        }
    }

    /// What the person reads on stderr; `None` when nothing should be said.
    pub(crate) fn message(&self) -> Option<String> {
        match self {
            Self::Closed => None,
            other @ (Self::Usage(_) | Self::Environment(_) | Self::SignedOut(_)) => {
                Some(format!("llx: {other}"))
            }
        }
    }

    /// A failed write to stdout: a closed pipe ends quietly, anything else is the environment.
    pub(crate) fn from_write(error: &io::Error) -> Self {
        if error.kind() == io::ErrorKind::BrokenPipe {
            Self::Closed
        } else {
            Self::Environment(format!("could not write the output: {error}"))
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) | Self::Environment(message) | Self::SignedOut(message) => {
                f.write_str(message)
            }
            Self::Closed => f.write_str("the output was closed"),
        }
    }
}

impl std::error::Error for Failure {}

/// Write `text` to stdout and flush it: a closed pipe or another write error is a [`Failure`],
/// never a panic (`println!` panics on a closed pipe).
pub(crate) fn print(text: &str) -> Result<(), Failure> {
    use io::Write;
    let mut out = io::stdout().lock();
    out.write_all(text.as_bytes())
        .and_then(|()| out.flush())
        .map_err(|error| Failure::from_write(&error))
}

/// Write `text` to stderr, for the person; if even that fails, no one is left to tell.
pub(crate) fn note(text: &str) {
    use io::Write;
    let mut err = io::stderr();
    let _ = err.write_all(text.as_bytes()).and_then(|()| err.flush());
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
