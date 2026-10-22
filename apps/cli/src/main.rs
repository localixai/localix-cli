//! llx: an ultrafast, lightweight coding agent for the terminal. It reads, edits and runs commands in a
//! workspace, and drives a native browser window whose clicks and typing a decision model does.
//! This binary parses the command line and runs a command. It exits here, or in the browser host
//! once that owns the main thread (its event loop never returns): there, with the code the run
//! returns. Nothing else exits.

mod brake;
mod cli;
mod commands;
mod compose;
mod context;
mod error;
mod exit;
mod gate;
mod input;
mod interrupt;
mod mode;
mod output;
mod person;
mod printable;
#[cfg(test)]
mod testing;
mod tui;

use clap::Parser;

fn main() {
    let cli = match cli::Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            // `--help` and `--version` come here too, and are no error; a closed pipe while they
            // print is no error either.
            let code = if error.use_stderr() {
                exit::ENVIRONMENT
            } else {
                exit::OK
            };
            let _ = error.print();
            std::process::exit(code);
        }
    };
    let code = match commands::dispatch(cli) {
        Ok(code) => code,
        Err(failure) => {
            if let Some(message) = failure.message() {
                use std::io::Write;
                let _ = writeln!(std::io::stderr(), "{message}");
            }
            failure.code()
        }
    };
    std::process::exit(code);
}
