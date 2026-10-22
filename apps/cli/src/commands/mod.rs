//! What each command does. Every command returns its exit code, or a [`Failure`] that `main`
//! reports; a run that hosts the browser never returns (its event loop owns the main thread and
//! exits the process with the run's code, which nothing else decides).

pub mod doctor;
pub mod login;
pub mod repl;
pub mod run;
pub mod sessions;
pub mod signin;

use crate::cli::{Cli, Command, ExecCommand, joined};
use crate::error::Failure;
use run::Kind;

pub fn dispatch(cli: Cli) -> Result<i32, Failure> {
    let kind = match cli.print {
        true => Kind::Exec,
        false => Kind::Session,
    };
    match cli.command {
        None if cli.resume_last => {
            sessions::resume(None, joined(&cli.prompt).as_deref(), cli.run, kind)
        }
        None => run::run(joined(&cli.prompt).as_deref(), cli.run, None, kind),
        Some(Command::Exec(exec)) => match exec.command {
            None => run::run(joined(&exec.prompt).as_deref(), exec.run, None, Kind::Exec),
            Some(ExecCommand::Resume(args)) => resumed(args, Kind::Exec),
        },
        Some(Command::Resume(args)) => resumed(args, Kind::Session),
        Some(Command::Sessions) => sessions::list(),
        Some(Command::Logout) => signin::logout(),
        Some(Command::Doctor(models)) => doctor::doctor(&models),
    }
}

fn resumed(args: crate::cli::ResumeArgs, kind: Kind) -> Result<i32, Failure> {
    let (id, prompt) = args.target().map_err(Failure::Usage)?;
    let (id, prompt) = (id.map(str::to_string), prompt.map(str::to_string));
    sessions::resume(id.as_deref(), prompt.as_deref(), args.run, kind)
}
