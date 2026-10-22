//! The command line: what `llx --help` shows. Parsing only; what each command does is in
//! `commands/`.

use std::path::PathBuf;

use clap::builder::RangedU64ValueParser;
use clap::{ArgGroup, Args, Parser, Subcommand};
use openrouter::system_one;

use crate::mode::Mode;
use crate::providers::Provider;

const USAGE: &str = "llx [OPTIONS] [PROMPT]\n       \
    llx exec [OPTIONS] [PROMPT | -]\n       \
    llx <COMMAND>";

const ABOUT: &str = "llx: an ultrafast, lightweight coding agent for the terminal";

const LONG_ABOUT: &str = "llx: an ultrafast, lightweight coding agent for the terminal.

It works in a directory (the current one, or -C DIR) with five tools: read, edit, bash (on \
unix it can keep a program that asks, such as a REPL, open in a terminal session until the task \
ends), a browser whose clicks and typing are done by a fast decision model (the agent also reads \
the page, runs scripts in it and sees it in screenshots), and ask, a question to you (in a \
session only).

With no command, llx opens a session at the terminal: PROMPT, when given, is its first message, \
and each line you type goes on from the last. `llx exec` (or -p) runs one task without a session \
and exits, with the answer alone on stdout: for scripts, CI and pipes.

To sign in, type /login in a session: choose a provider (OpenAI, signed in with ChatGPT in \
your browser, or OpenRouter, with a key), then one of its models and a reasoning level that \
model takes. The choice is saved as the default for later sessions and for exec. The browser \
tool's decision model runs on OpenRouter, so it needs an OpenRouter key whichever provider the \
agent thinks with.

The mode (--mode) decides what runs without asking. In default, commands, file changes and \
browser actions are asked for at the terminal (y, n, or a: always for this tool), and refused \
outside one. plan investigates and answers with a plan: file changes are refused, and each \
command (and each input to a terminal session) is asked. bypass (-y) allows everything. \
autonomous also allows everything, and the agent asks you nothing (in exec, where ask is not \
offered, it is the same as bypass). In a session, /mode switches it, /plan enters plan mode, \
Shift+Tab then Enter toggles plan mode, and /status shows the mode.

Ctrl-C stops the task (the command running and every terminal session are killed, and an open \
question or approval ends); a second Ctrl-C quits.";

const EXAMPLES: &str = "Examples:
  llx                                              a session at the terminal
  llx \"fix the failing test in src/parse.rs\"       a session that starts with this task
  llx -c                                           go on with the last session here
  llx --mode plan \"how should we split src/parse.rs?\"
                                                  a session that plans before it changes anything
  llx exec -y \"run the tests and fix what fails\"   one task, then exit
  git diff | llx -p \"review this change\"           stdin is added to the prompt
  llx exec - < task.md                             the prompt from stdin
  llx exec --json -y \"update the changelog\" | jq -c 'select(.type == \"tool.call\")'
  llx exec --schema '{\"type\":\"object\",\"properties\":{\"version\":{\"type\":\"string\"}}}' \\
      \"which version is this crate?\"
  llx exec resume --last \"now update the README\"

Exit codes: 0 achieved, 1 not achieved, 2 out of turns or time, 3 environment or usage,
130 interrupted.

Environment: OPENROUTER_API_KEY (else the key /login saved), LLX_PROVIDER, LLX_MODEL,
LLX_REASONING, LLX_DECISION_MODEL, LLX_SESSIONS (where sessions are kept, default
~/.lx/sessions). Sign-ins and the /login choice are kept in ~/.config/llx/ (0600).";

#[derive(Parser, Debug)]
#[command(
    name = "llx",
    version,
    about = ABOUT,
    long_about = LONG_ABOUT,
    override_usage = USAGE,
    after_long_help = EXAMPLES,
    args_conflicts_with_subcommands = true
)]
pub struct Cli {
    /// The first message of the session; with -p, the task (`-` reads it from stdin). Quotes
    /// are optional: the words are joined with spaces.
    #[arg(value_name = "PROMPT", num_args = 1..)]
    pub prompt: Vec<String>,
    /// Run the prompt as one task and exit, as `llx exec` does.
    #[arg(short = 'p', long)]
    pub print: bool,
    /// Go on with the newest session of this directory, as `llx resume --last` does.
    #[arg(short = 'c', long = "continue")]
    pub resume_last: bool,
    #[command(flatten)]
    pub run: RunOptions,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run one task without a session and exit; the answer alone goes to stdout.
    ///
    /// Given a prompt, stdin that is not a terminal is read to its end and added in a <stdin>
    /// block (`</dev/null` skips it); `-` reads the prompt itself from stdin.
    #[command(visible_alias = "e", args_conflicts_with_subcommands = true)]
    Exec(Box<ExecArgs>),
    /// Go on with a saved session, at the terminal.
    ///
    /// Its conversation is not kept, only each task and how it ended: the agent starts a new
    /// conversation that is told them, and records into the same session.
    #[command(group(ArgGroup::new("which").args(["id", "last"]).required(true).multiple(true)))]
    Resume(ResumeArgs),
    /// List saved sessions, newest first.
    Sessions,
    /// Forget every saved sign-in: the ChatGPT tokens and the OpenRouter key.
    Logout,
    /// Check the sign-ins, both models, the browser's WebView and the shell.
    Doctor(ModelOptions),
}

#[derive(Args, Debug)]
pub struct ExecArgs {
    /// The task, in plain words, quoted or not; `-` reads it from stdin. Without one, piped
    /// stdin is the task.
    #[arg(value_name = "PROMPT", num_args = 1..)]
    pub prompt: Vec<String>,
    #[command(flatten)]
    pub run: RunOptions,
    #[command(subcommand)]
    pub command: Option<ExecCommand>,
}

#[derive(Subcommand, Debug)]
pub enum ExecCommand {
    /// Go on with a saved session with one task, then exit.
    #[command(group(ArgGroup::new("which").args(["id", "last"]).required(true).multiple(true)))]
    Resume(ResumeArgs),
}

#[derive(Args, Debug)]
pub struct ResumeArgs {
    /// The session to continue (see `llx sessions`).
    #[arg(value_name = "ID")]
    pub id: Option<String>,
    /// Continue the newest session; the first argument is then the prompt.
    #[arg(long)]
    pub last: bool,
    /// The next message; with `exec resume`, the task.
    #[arg(value_name = "PROMPT")]
    pub prompt: Option<String>,
    #[command(flatten)]
    pub run: RunOptions,
}

impl ResumeArgs {
    /// The session asked for (`None`: the newest) and the prompt. With `--last` the first
    /// argument is the prompt, as `codex resume --last` reads it.
    pub fn target(&self) -> Result<(Option<&str>, Option<&str>), String> {
        match (self.last, self.id.as_deref(), self.prompt.as_deref()) {
            (true, Some(_), Some(_)) => {
                Err("with --last, give only the prompt: llx resume --last \"<task>\"".into())
            }
            (true, first, None) => Ok((None, first)),
            (true, None, prompt) => Ok((None, prompt)),
            (false, id, prompt) => Ok((id, prompt)),
        }
    }
}

/// The two models a run uses. The chat model's provider, model and reasoning level default to
/// what `/login` saved.
#[derive(Args, Debug, Clone)]
#[command(next_help_heading = "Model")]
pub struct ModelOptions {
    /// Where the agent's chat model comes from: openai (a ChatGPT sign-in) or openrouter (a key).
    /// Default: the one /login saved, else the one signed in to.
    #[arg(long, value_enum, value_name = "PROVIDER", env = "LLX_PROVIDER")]
    pub provider: Option<Provider>,
    /// The agent's chat model, a model id of the provider's (any OpenRouter id works). Default:
    /// the one /login saved, else the provider's default.
    #[arg(short = 'm', long, value_name = "MODEL", env = "LLX_MODEL")]
    pub model: Option<String>,
    /// How much the chat model reasons: off, minimal, low, medium, high, xhigh, max or ultra,
    /// as far as the model takes it (/login lists its levels). Default: the model's own.
    #[arg(long, value_name = "LEVEL", env = "LLX_REASONING")]
    pub reasoning: Option<String>,
    /// The decision model that clicks and types in the browser.
    #[arg(long, value_name = "MODEL", env = "LLX_DECISION_MODEL", default_value = system_one::DEFAULT_MODEL)]
    pub decision_model: String,
}

/// How a task runs: shared by a session, `exec` and `resume`. The output options answer one task,
/// so they need `exec` or -p.
#[derive(Args, Debug, Clone)]
pub struct RunOptions {
    #[command(flatten)]
    pub models: ModelOptions,

    /// The directory the agent works in: paths resolve in it and files change only inside it.
    /// Default: the current directory.
    #[arg(
        short = 'C',
        long = "cd",
        value_name = "DIR",
        help_heading = "Workspace"
    )]
    pub cd: Option<PathBuf>,

    /// Pictures (PNG, JPEG, GIF, WebP) attached to the first task, as the person's own: a
    /// mockup, a screenshot of a bug. Repeat it, or separate them with commas. In a session,
    /// drag a picture in, paste its path, or press Ctrl+V for the clipboard's.
    #[arg(
        short = 'i',
        long = "image",
        value_name = "FILE",
        value_delimiter = ',',
        help_heading = "Workspace"
    )]
    pub images: Vec<PathBuf>,

    /// Allow commands, file changes and browser actions without asking: --mode bypass. Also
    /// accepted as --yolo and --dangerously-skip-permissions.
    #[arg(short = 'y', long, help_heading = "Approvals",
        aliases = ["yolo", "dangerously-skip-permissions"])]
    pub yes: bool,
    /// What runs without asking, what is refused, and whether the agent may ask you questions.
    /// Default: default. Also accepted as --permission-mode.
    #[arg(
        long,
        value_enum,
        value_name = "MODE",
        help_heading = "Approvals",
        alias = "permission-mode",
        conflicts_with = "yes"
    )]
    pub mode: Option<Mode>,

    /// Print events as JSON lines on stdout: run.started, tool.call, tool.result, step, turn,
    /// run.completed. With `exec` or -p.
    #[arg(long, help_heading = "Output")]
    pub json: bool,
    /// Also write the final answer to FILE (the data, with --schema; empty when none matched).
    /// With `exec` or -p.
    #[arg(short = 'o', long, value_name = "FILE", help_heading = "Output")]
    pub output_last_message: Option<PathBuf>,
    /// End with data matching this JSON Schema (inline, or @file), printed alone on stdout.
    /// With `exec` or -p.
    #[arg(long, value_name = "SCHEMA", help_heading = "Output")]
    pub schema: Option<String>,
    /// Show each result and each model turn in the trace; with --json, the trace on stderr too.
    #[arg(short, long, help_heading = "Output")]
    pub verbose: bool,

    /// Model turns for one task (1 to 10000).
    #[arg(long, value_name = "N", default_value_t = 50, help_heading = "Limits",
        value_parser = RangedU64ValueParser::<usize>::new().range(1..=10_000))]
    pub max_turns: usize,
    /// Actions for one browser task of the decision model (1 to 10000).
    #[arg(long, value_name = "N", default_value_t = 40, help_heading = "Limits",
        value_parser = RangedU64ValueParser::<usize>::new().range(1..=10_000))]
    pub max_steps: usize,
    /// Seconds one task may take, up to a day; it stops at the first model turn past them.
    #[arg(long, value_name = "SECONDS", default_value_t = 1800, help_heading = "Limits",
        value_parser = RangedU64ValueParser::<u64>::new().range(1..=86_400))]
    pub timeout: u64,

    /// Always show the browser window. By default it shows in a session (except in autonomous
    /// mode) and stays hidden in exec; the agent can show, hide or close it per call. It is an
    /// ordinary window, never on top of others, and never takes the keyboard.
    #[arg(long, help_heading = "Browser")]
    pub show_browser: bool,
}

/// A prompt given as words, unquoted or quoted, as one text; `None` when none was given.
pub fn joined(words: &[String]) -> Option<String> {
    (!words.is_empty()).then(|| words.join(" "))
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
