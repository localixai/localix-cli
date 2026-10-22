//! The command line: what `lxsurf --help` shows.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use openrouter::{chat, system_one};

#[derive(Parser)]
#[command(
    name = "lxsurf",
    version,
    about = "lxsurf: a goal in, an agent surfs a native browser window there and answers"
)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
    #[command(flatten)]
    pub run: RunArgs,
}

#[derive(Subcommand)]
pub enum Command {
    /// Test mode: print PASS or FAIL and exit 0 or 1.
    Check(Box<RunArgs>),
    /// Sign in with OpenRouter in your browser and save the key it issues.
    Login,
    /// Forget the saved key.
    Logout,
    /// List OpenRouter's decision models, cheapest first.
    Models,
    /// Check the key, both models and the WebView runtime.
    Doctor,
    /// List saved sessions, newest first (`--resume <ID>` continues one).
    Sessions,
    /// Guides for AI agents: `skills list`, `skills get core`.
    Skills {
        #[command(subcommand)]
        action: SkillsAction,
    },
}

#[derive(Subcommand)]
pub enum SkillsAction {
    /// List the guides.
    List,
    /// Print a guide.
    Get { name: String },
}

#[derive(Args, Clone)]
pub struct RunArgs {
    /// What to do or find, in plain words; `-` reads it from stdin.
    pub goal: Option<String>,
    /// A statement that must hold on the final page; repeatable. Any false one fails the run.
    #[arg(long = "expect", value_name = "STATEMENT")]
    pub expects: Vec<String>,
    /// Answer as JSON matching this JSON Schema (inline, or @file), alone on stdout.
    #[arg(long, value_name = "SCHEMA")]
    pub schema: Option<String>,
    /// Keep the window invisible. Default under CI or when not in a terminal; set
    /// LXSURF_HEADLESS=1 to make it the default everywhere.
    #[arg(long, env = "LXSURF_HEADLESS", value_parser = clap::builder::BoolishValueParser::new())]
    pub headless: bool,
    /// Show the window even where it would be invisible by default (wins over --headless).
    #[arg(long)]
    pub show: bool,
    /// The agent's chat model (an OpenRouter model id).
    #[arg(long, env = "LXSURF_MODEL", default_value = chat::DEFAULT_MODEL)]
    pub model: String,
    /// The decision model that clicks and types (an OpenRouter decision model; see `models`).
    #[arg(long, env = "LXSURF_DECISION_MODEL", default_value = system_one::DEFAULT_MODEL)]
    pub decision_model: String,
    /// No agent: the decision model takes the whole goal itself, from the page it names, typing
    /// only the goal's own words (quote what to type).
    #[arg(long, conflicts_with = "schema")]
    pub no_llm: bool,
    /// The agent's turns for one goal.
    #[arg(long, default_value_t = 30)]
    pub max_turns: usize,
    /// Actions for one task of the decision model (with --no-llm, for the whole goal).
    #[arg(long, default_value_t = 40)]
    pub max_steps: usize,
    /// Give up after this many seconds.
    #[arg(long, default_value_t = 600)]
    pub timeout: u64,
    /// Print one JSON object on stdout (schema 3).
    #[arg(long)]
    pub json: bool,
    /// Save a picture of the final page (PNG; macOS).
    #[arg(long, value_name = "FILE")]
    pub screenshot: Option<PathBuf>,
    /// Write the full trace as JSON to this file.
    #[arg(long, value_name = "FILE")]
    pub trace: Option<PathBuf>,
    /// Leave the window open after the run: in a terminal, type the next goal to continue in the
    /// same window, conversation and session (Ctrl-D or /exit quits); otherwise wait until the
    /// window is closed.
    #[arg(long)]
    pub keep_open: bool,
    /// Continue the newest saved session, on the page it left.
    #[arg(short = 'c', long = "continue", conflicts_with = "resume")]
    pub continue_last: bool,
    /// Continue the saved session with this id (see `sessions`).
    #[arg(long, value_name = "ID")]
    pub resume: Option<String>,
    /// Open the page the goal names and print what the decision model would read, then exit; no
    /// key needed.
    #[arg(long)]
    pub dry_run: bool,
    /// Trace on stderr even with --json.
    #[arg(short, long)]
    pub verbose: bool,
}
