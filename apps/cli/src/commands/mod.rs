//! What each command does. Every command returns its exit code, or a [`Failure`] that `main`
//! reports; a run that hosts the browser never returns (its event loop owns the main thread and
//! exits the process with the run's code, which nothing else decides).

pub mod doctor;
pub mod login;
pub mod repl;
pub mod run;
pub mod sessions;

use crate::cli::{Cli, Command, ExecCommand};
use crate::error::Failure;
use run::Mode;

pub fn dispatch(cli: Cli) -> Result<i32, Failure> {
    let mode = match cli.print {
        true => Mode::Exec,
        false => Mode::Session,
    };
    match cli.command {
        None if cli.resume_last => sessions::resume(None, cli.prompt.as_deref(), cli.run, mode),
        None => run::run(cli.prompt.as_deref(), cli.run, None, mode),
        Some(Command::Exec(exec)) => match exec.command {
            None => run::run(exec.prompt.as_deref(), exec.run, None, Mode::Exec),
            Some(ExecCommand::Resume(args)) => resumed(args, Mode::Exec),
        },
        Some(Command::Resume(args)) => resumed(args, Mode::Session),
        Some(Command::Sessions) => sessions::list(),
        Some(Command::Login(args)) => login::login(args),
        Some(Command::Logout) => login::logout(),
        Some(Command::Doctor(models)) => doctor::doctor(&models),
    }
}

fn resumed(args: crate::cli::ResumeArgs, mode: Mode) -> Result<i32, Failure> {
    let (id, prompt) = args.target().map_err(Failure::Usage)?;
    let (id, prompt) = (id.map(str::to_string), prompt.map(str::to_string));
    sessions::resume(id.as_deref(), prompt.as_deref(), args.run, mode)
}
