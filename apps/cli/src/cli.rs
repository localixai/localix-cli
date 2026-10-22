//! The command line: what `lx --help` shows. Parsing only; what each command does is in
//! `commands/`.

use std::path::PathBuf;

use clap::builder::RangedU64ValueParser;
use clap::{ArgGroup, Args, Parser, Subcommand};
use openrouter::{chat, system_one};

const USAGE: &str = "lx [OPTIONS] [PROMPT]\n       \
    lx [OPTIONS] - < prompt.txt\n       \
    lx <COMMAND>";

const ABOUT: &str = "lx: Localix's ultrafast, lightweight coding agent for the terminal";

const LONG_ABOUT: &str = "lx: Localix's ultrafast, lightweight coding agent for the terminal.

It works in a directory (the current one, or -C DIR) with four tools: read, edit, bash and a \
browser whose clicks and typing are done by a fast decision model. With a PROMPT it runs the task \
and exits; without one, in a terminal, it opens a prompt where each line goes on from the last.

Commands, file changes and browser actions are asked for at the terminal (y, n, or a: always \
for this tool); -y allows them without asking. Outside a terminal they are refused unless -y is \
given. Ctrl-C stops the task (the command running is killed); a second Ctrl-C quits.";

const EXAMPLES: &str = "Examples:
  lx \"fix the failing test in src/parse.rs\"
  lx -y \"run the tests and fix what fails\"
  git diff | lx \"review this change\"          stdin is added to the prompt
  lx - < task.md                              the prompt from stdin
  lx -C ../site \"open http://localhost:3000 and check that sign-up works\"
  lx --json -y \"update the changelog\" | jq -c 'select(.type == \"tool.call\")'
  lx --schema '{\"type\":\"object\",\"properties\":{\"version\":{\"type\":\"string\"}}}' \\
      \"which version is this crate?\"
  lx                                          a prompt in the terminal
  lx resume --last \"now update the README\"

Exit codes: 0 achieved, 1 not achieved, 2 out of turns or time, 3 environment or usage,
130 interrupted.

Environment: OPENROUTER_API_KEY (else the key `lx login` saved), LX_MODEL,
LX_DECISION_MODEL, LX_SESSIONS (where sessions are kept, default ~/.lx/sessions).";

#[derive(Parser, Debug)]
#[command(
    name = "lx",
    version,
    about = ABOUT,
    long_about = LONG_ABOUT,
    override_usage = USAGE,
    after_long_help = EXAMPLES,
    args_conflicts_with_subcommands = true
)]
pub struct Cli {
    /// The task, in plain words; `-` reads it from stdin. Given a task, stdin that is not a
    /// terminal is read to its end and added in a <stdin> block (`</dev/null` skips it). With
    /// neither, a terminal opens a prompt line.
    #[arg(value_name = "PROMPT")]
    pub prompt: Option<String>,
    #[command(flatten)]
    pub run: RunOptions,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Sign in with OpenRouter in your browser and save the key it issues.
    Login(LoginArgs),
    /// Forget the key `login` saved.
    Logout,
    /// List saved sessions, newest first.
    Sessions,
    /// Continue a saved session.
    ///
    /// Its conversation is not kept, only each task and how it ended: the agent starts a new
    /// conversation that is told them, and records into the same session.
    #[command(group(ArgGroup::new("which").args(["id", "last"]).required(true).multiple(true)))]
    Resume(ResumeArgs),
    /// Check the key, both models, the browser's WebView and the shell.
    Doctor(ModelOptions),
}

#[derive(Args, Debug)]
#[command(args_conflicts_with_subcommands = true)]
pub struct LoginArgs {
    /// Read the key from stdin instead of signing in: `printenv KEY | lx login --with-token`.
    #[arg(long)]
    pub with_token: bool,
    #[command(subcommand)]
    pub action: Option<LoginAction>,
}

#[derive(Subcommand, Debug)]
pub enum LoginAction {
    /// Where the key comes from, masked; exit 0 when signed in, 3 when not.
    Status,
}

#[derive(Args, Debug)]
pub struct ResumeArgs {
    /// The session to continue (see `lx sessions`).
    #[arg(value_name = "ID")]
    pub id: Option<String>,
    /// Continue the newest session; the first argument is then the prompt.
    #[arg(long)]
    pub last: bool,
    /// The next task; without one, a terminal opens a prompt line.
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
                Err("with --last, give only the prompt: lx resume --last \"<task>\"".into())
            }
            (true, first, None) => Ok((None, first)),
            (true, None, prompt) => Ok((None, prompt)),
            (false, id, prompt) => Ok((id, prompt)),
        }
    }
}

/// The two models a run uses.
#[derive(Args, Debug, Clone)]
#[command(next_help_heading = "Model")]
pub struct ModelOptions {
    /// The agent's chat model, an OpenRouter model id.
    #[arg(short = 'm', long, value_name = "MODEL", env = "LX_MODEL", default_value = chat::DEFAULT_MODEL)]
    pub model: String,
    /// The decision model that clicks and types in the browser.
    #[arg(long, value_name = "MODEL", env = "LX_DECISION_MODEL", default_value = system_one::DEFAULT_MODEL)]
    pub decision_model: String,
}

/// How a task runs: shared by a run, the prompt line and `resume`.
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

    /// Allow commands, file changes and browser actions without asking.
    #[arg(short = 'y', long, help_heading = "Approvals")]
    pub yes: bool,

    /// Print events as JSON lines on stdout: run.started, tool.call, tool.result, step, turn,
    /// run.completed.
    #[arg(long, help_heading = "Output")]
    pub json: bool,
    /// Also write the final answer to FILE (the data, with --schema; empty when none matched).
    #[arg(short = 'o', long, value_name = "FILE", help_heading = "Output")]
    pub output_last_message: Option<PathBuf>,
    /// End with data matching this JSON Schema (inline, or @file), printed alone on stdout.
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

    /// Show the browser window (it is invisible by default, and opens only when the agent first
    /// uses the browser).
    #[arg(long, help_heading = "Browser")]
    pub show_browser: bool,
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
