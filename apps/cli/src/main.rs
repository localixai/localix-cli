//! llx: an ultrafast, lightweight coding agent for the terminal. It reads, edits and runs commands in a
//! workspace, and drives a native browser window whose clicks and typing a decision model does.
//! This binary parses the command line and runs a command. It exits here, or in the browser host
//! once that owns the main thread (its event loop never returns): there, with the code the run
//! returns. Nothing else exits.

mod agents;
mod arrivals;
mod attach;
mod brake;
mod checkpoints;
mod choice;
mod cli;
#[cfg(unix)]
mod colours;
mod commands;
mod compose;
mod context;
mod desk;
mod error;
mod exit;
mod gate;
mod input;
mod instructions;
mod interrupt;
mod jobs;
mod kinds;
mod live;
mod meta;
mod mode;
mod output;
mod picker;
mod printable;
mod providers;
mod server;
#[cfg(test)]
mod testing;
mod tui;
mod windows;
#[cfg(unix)]
mod xterm;

use clap::Parser;

#[allow(
    clippy::exit,
    clippy::disallowed_methods,
    reason = "the one place the process ends, with the exit-code contract (exit.rs)"
)]
fn main() {
    std::process::exit(run());
}

/// The command line parsed and its command run; the process's exit code.
fn run() -> i32 {
    let cli = match cli::Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            // `--help` and `--version` come here too, and are no error; a closed pipe while they
            // print is no error either.
            let _ = error.print();
            return if error.use_stderr() {
                exit::ENVIRONMENT
            } else {
                exit::OK
            };
        }
    };
    match commands::dispatch(cli) {
        Ok(code) => code,
        Err(failure) => {
            if let Some(message) = failure.message() {
                use std::io::Write;
                let _ = writeln!(std::io::stderr(), "{message}");
            }
            failure.code()
        }
    }
}
