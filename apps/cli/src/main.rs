//! decision-loop: a goal in, a decision model drives a tool there. The composition root: the one
//! place that builds the packages (a decision-model provider, the tools, the history store),
//! registers the tools and hands everything to the loop through its ports.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use agent::history::History;
use clap::{Args, Parser, Subcommand};
use decision_loop::toolbox::Toolbox;
use openrouter::App;
use openrouter::client::OpenRouter;
use openrouter::system_one::{self, SystemOne};
use session::Sessions;
use tools::browser::{self, BrowserTool};

mod chat;
mod doctor;
mod exit;
mod report;
mod run;
mod skills;
mod start;

use chat::Deadline;

/// Who calls OpenRouter: its attribution, the sign-in page, the folder the key is saved in.
pub const APP: App = App {
    name: "Decision Loop",
    slug: "decision-loop",
    version: env!("CARGO_PKG_VERSION"),
};

#[derive(Parser)]
#[command(
    name = "decision-loop",
    version,
    about = "Decision Loop: a goal in, a decision model drives a tool there"
)]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[command(flatten)]
    run: RunArgs,
}

#[derive(Subcommand)]
enum Command {
    /// Test mode: print PASS or FAIL and exit 0 or 1.
    Check(Box<RunArgs>),
    /// Sign in with OpenRouter in your browser and save the key it issues.
    Login,
    /// Forget the saved key.
    Logout,
    /// List OpenRouter's decision models, cheapest first.
    Models,
    /// Check the key, the decision model and the WebView runtime.
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
enum SkillsAction {
    /// List the guides.
    List,
    /// Print a guide.
    Get { name: String },
}

#[derive(Args, Clone)]
struct RunArgs {
    /// What to do, in plain words (name the site or file it starts from); `-` reads it from stdin.
    goal: Option<String>,
    /// A statement that must hold on the final page; repeatable. Any false one fails the run.
    #[arg(long = "expect", value_name = "STATEMENT")]
    expects: Vec<String>,
    /// Keep tools' windows invisible. Default under CI or when not in a terminal; set
    /// DECISION_LOOP_HEADLESS=1 to make it the default everywhere.
    #[arg(long, env = "DECISION_LOOP_HEADLESS", value_parser = clap::builder::BoolishValueParser::new())]
    headless: bool,
    /// Show tools' windows even where they would be invisible by default (wins over --headless).
    #[arg(long)]
    show: bool,
    /// Decision model (an OpenRouter decision model id; see `models`).
    #[arg(long, env = "DECISION_LOOP_MODEL", default_value = system_one::DEFAULT_MODEL)]
    model: String,
    /// Actions for the whole goal.
    #[arg(long, default_value_t = 100)]
    max_steps: usize,
    /// Give up after this many seconds.
    #[arg(long, default_value_t = 600)]
    timeout: u64,
    /// Print one JSON object on stdout (schema 2).
    #[arg(long)]
    json: bool,
    /// Save a picture of what the tool shows at the end (PNG; the browser on macOS).
    #[arg(long, value_name = "FILE")]
    screenshot: Option<PathBuf>,
    /// Write the full trace as JSON to this file.
    #[arg(long, value_name = "FILE")]
    trace: Option<PathBuf>,
    /// Leave the window open after the run: in a terminal, type the next goal to continue in the
    /// same window and session (Ctrl-D or /exit quits); otherwise wait until the window is closed.
    #[arg(long)]
    keep_open: bool,
    /// Continue the newest saved session, where it left its tool.
    #[arg(short = 'c', long = "continue", conflicts_with = "resume")]
    continue_last: bool,
    /// Continue the saved session with this id (see `sessions`).
    #[arg(long, value_name = "ID")]
    resume: Option<String>,
    /// Start every tool for the goal, print what each shows (what the decision model would read),
    /// and exit; no key needed.
    #[arg(long)]
    dry_run: bool,
    /// Trace on stderr even with --json.
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        None => surf(cli.run, false),
        Some(Command::Check(args)) => surf(*args, true),
        Some(Command::Login) => match openrouter::auth::login(APP) {
            Ok(path) => println!("Signed in. The key is saved in {}", path.display()),
            Err(error) => quit(exit::ENVIRONMENT, &error),
        },
        Some(Command::Logout) => match openrouter::auth::logout(APP) {
            Ok(true) => println!("Signed out."),
            Ok(false) => println!("No saved key."),
            Err(error) => quit(exit::ENVIRONMENT, &error.to_string()),
        },
        Some(Command::Models) => std::process::exit(doctor::models()),
        Some(Command::Doctor) => std::process::exit(doctor::doctor(
            &std::env::var("DECISION_LOOP_MODEL")
                .unwrap_or_else(|_| system_one::DEFAULT_MODEL.into()),
        )),
        Some(Command::Sessions) => sessions(),
        Some(Command::Skills {
            action: SkillsAction::List,
        }) => print!("{}", skills::list()),
        Some(Command::Skills {
            action: SkillsAction::Get { name },
        }) => match skills::get(&name) {
            Some(text) => print!("{text}"),
            None => quit(
                exit::ENVIRONMENT,
                &format!("no guide named {name}; see `decision-loop skills list`"),
            ),
        },
    }
}

/// Where sessions are kept: `DECISION_LOOP_SESSIONS`, else `~/.decision-loop/sessions`.
fn sessions_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("DECISION_LOOP_SESSIONS").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .unwrap_or_default();
    PathBuf::from(home).join(".decision-loop/sessions")
}

fn surf(mut args: RunArgs, check: bool) -> ! {
    let history = Sessions {
        dir: sessions_dir(),
    };
    let resumed = (args.continue_last || args.resume.is_some()).then(|| {
        history
            .load(args.resume.as_deref())
            .unwrap_or_else(|message| quit(exit::ENVIRONMENT, &message))
    });
    // Without a goal, a resumed session or a kept-open run waits for one in the chat.
    let goal = match (&resumed, args.goal.as_deref()) {
        (_, None) if (resumed.is_some() || args.keep_open) && chat::interactive(args.json) => {
            args.keep_open = true;
            None
        }
        _ => Some(
            start::read_goal(args.goal.as_deref())
                .unwrap_or_else(|message| quit(exit::ENVIRONMENT, &message)),
        ),
    };
    let key = if args.dry_run {
        String::new()
    } else {
        let Some((key, _)) = openrouter::auth::key(APP) else {
            quit(
                exit::ENVIRONMENT,
                "no OpenRouter key: set OPENROUTER_API_KEY or run `decision-loop login`",
            )
        };
        key
    };
    let headless = if args.show {
        false
    } else if args.headless {
        true
    } else {
        start::headless_by_default()
    };
    if !browser::has_display() {
        quit(
            exit::ENVIRONMENT,
            "no display for the browser window: run under `xvfb-run decision-loop …`",
        );
    }
    // A continued session resumes where its last goal left its tool.
    let resume_at = resumed
        .as_ref()
        .map(|session| session.location.clone())
        .filter(|location| !location.is_empty());
    let timeout = Duration::from_secs(args.timeout);
    let deadline = Deadline::start();
    browser::host(headless, exit::INTERRUPTED, move |host| {
        let toolbox = Toolbox::default();
        toolbox.add(Arc::new(BrowserTool::new(host, headless)));
        if args.dry_run {
            return dry_run(
                &args,
                &toolbox,
                goal.as_deref().unwrap_or(""),
                resume_at.as_deref(),
            );
        }
        let client = OpenRouter::new(key, APP);
        let model = SystemOne {
            client: &client,
            model: args.model.clone(),
        };
        let mut session = resumed.unwrap_or_else(|| history.start());
        let mut options = run::Options {
            goal: goal.clone().unwrap_or_default(),
            decision_model: args.model.clone(),
            expects: args.expects.clone(),
            max_steps: args.max_steps,
            json: args.json,
            check,
            trace: args.trace.clone(),
            screenshot: args.screenshot.clone(),
            resume_at: resume_at.clone(),
            verbose: args.verbose,
        };
        let turn = |options: &run::Options, session: &mut agent::history::Session| {
            chat::turn(
                options, &toolbox, &model, &history, session, &deadline, timeout,
            )
        };
        let mut code = match goal {
            Some(_) => turn(&options, &mut session),
            None => exit::OK,
        };
        // Only the first goal resumes; the next ones go on from where the last one left off.
        options.resume_at = None;
        if !args.keep_open {
            return code;
        }
        if !chat::interactive(args.json) {
            eprintln!("decision-loop: done (exit {code}); close the window to quit");
            loop {
                std::thread::park();
            }
        }
        // Follow-ups are plain goals: the expectations were the first goal's.
        options.expects.clear();
        options.check = false;
        options.trace = None;
        options.screenshot = None;
        options.resume_at = None;
        eprintln!("decision-loop: type the next goal, or /help (Ctrl-D to quit)");
        while let Some(next) = chat::next_line() {
            if chat::command(&next, &toolbox, &session) {
                continue;
            }
            options.goal = next;
            code = turn(&options, &mut session);
        }
        code
    })
}

fn sessions() {
    let all = Sessions {
        dir: sessions_dir(),
    }
    .list();
    if all.is_empty() {
        println!("No saved sessions.");
        return;
    }
    for session in all.iter().take(30) {
        let first = session.turns.first().map_or("", |turn| turn.goal.as_str());
        let first: String = first.chars().take(60).collect();
        println!(
            "{}  {:>10}  {} turns  {first}  {}",
            session.id,
            session::ago(session.updated),
            session.turns.len(),
            session.location
        );
    }
}

/// Start every tool for `goal` and print what each shows: what the decision model would read.
fn dry_run(args: &RunArgs, toolbox: &Toolbox, goal: &str, at: Option<&str>) -> i32 {
    let mut code = exit::OK;
    for tool in toolbox.tools() {
        match tool.start(goal, at).and_then(|()| tool.observe()) {
            Ok(view) if args.json => {
                println!("{}", serde_json::json!({"tool": tool.name(), "view": view}))
            }
            Ok(view) => print!("== {}\n{}", tool.name(), report::view_table(&view)),
            Err(error) => {
                eprintln!("decision-loop: {}: {error}", tool.name());
                code = exit::ENVIRONMENT;
            }
        }
    }
    code
}

fn quit(code: i32, message: &str) -> ! {
    eprintln!("decision-loop: {message}");
    std::process::exit(code)
}
