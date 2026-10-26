//! What each command does. Every command returns its exit code, or a [`Failure`] that `main`
//! reports; a run that hosts the browser never returns (its event loop owns the main thread and
//! exits the process with the run's code, which nothing else decides).

pub(crate) mod decide;
pub(crate) mod doctor;
pub(crate) mod ended;
pub(crate) mod entry;
pub(crate) mod extras;
pub(crate) mod login;
pub(crate) mod model;
pub(crate) mod repl;
pub(crate) mod rewind;
pub(crate) mod run;
#[cfg(unix)]
pub(crate) mod serve;
pub(crate) mod sessions;
pub(crate) mod signin;
#[cfg(unix)]
pub(crate) mod window;

use crate::cli::{Cli, Command, ExecCommand, joined};
use crate::error::Failure;
use run::{At, Kind};

/// Run what `cli` asks for, a session's shown `at`.
pub(crate) fn dispatch(cli: Cli, at: At) -> Result<i32, Failure> {
    let kind = match cli.print {
        true => Kind::Exec,
        false => Kind::Session,
    };
    match cli.command {
        None if cli.resume_last => {
            sessions::resume(None, joined(&cli.prompt).as_deref(), cli.run, kind, at)
        }
        None => run::run(joined(&cli.prompt).as_deref(), cli.run, None, kind, at),
        Some(Command::Exec(exec)) => match exec.command {
            None => run::run(
                joined(&exec.prompt).as_deref(),
                exec.run,
                None,
                Kind::Exec,
                at,
            ),
            Some(ExecCommand::Resume(args)) => resumed(args, Kind::Exec, at),
        },
        Some(Command::Decide(args)) => decide::decide(&args),
        Some(Command::Resume(args)) => resumed(*args, Kind::Session, at),
        Some(Command::Sessions) => sessions::list(),
        Some(Command::Logout) => signin::logout(),
        Some(Command::Doctor(models)) => doctor::doctor(&models),
        #[cfg(unix)]
        Some(Command::Serve(serve)) => serve::serve(serve.action.as_ref()),
        #[cfg(unix)]
        Some(Command::Window(args)) => window::window(&args),
    }
}

fn resumed(args: crate::cli::ResumeArgs, kind: Kind, at: At) -> Result<i32, Failure> {
    let (id, prompt) = args.target();
    let id = id.map(str::to_string);
    sessions::resume(id.as_deref(), prompt.as_deref(), args.run, kind, at)
}
