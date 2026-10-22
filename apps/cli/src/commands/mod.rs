//! What each command does. Every command returns its exit code, or a [`Failure`] that `main`
//! reports; a run that hosts the browser never returns (its event loop owns the main thread and
//! exits the process with the run's code, which nothing else decides).

pub mod doctor;
pub mod login;
pub mod repl;
pub mod run;
pub mod sessions;

use crate::cli::{Cli, Command};
use crate::error::Failure;

pub fn dispatch(cli: Cli) -> Result<i32, Failure> {
    match cli.command {
        None => run::run(cli.prompt.as_deref(), cli.run, None),
        Some(Command::Login(args)) => login::login(args),
        Some(Command::Logout) => login::logout(),
        Some(Command::Sessions) => sessions::list(),
        Some(Command::Resume(args)) => sessions::resume(args),
        Some(Command::Doctor(models)) => doctor::doctor(&models),
    }
}
