//! lxsurf: a goal in, an agent surfs a native browser window there and answers. The composition
//! root: the one place that builds the packages (the models, the browser, the hands, the history
//! store) and hands them to the agent through its ports.

use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use contracts::history::History;
use contracts::view::View;
use openrouter::App;
use openrouter::chat::ChatModel;
use openrouter::client::OpenRouter;
use openrouter::system_one::{self, SystemOne};
use session::Sessions;
use tools::browser::{self, Page, SEARCH_PAGE, url_in};

mod chat;
mod cli;
mod doctor;
mod exit;
mod report;
mod run;
mod skills;
mod start;

use chat::{Chat, Deadline};
use cli::{Cli, Command, RunArgs, SkillsAction};

/// Who calls OpenRouter: its attribution, the sign-in page, the folder the key is saved in.
pub const APP: App = App {
    name: "lxsurf",
    slug: "lxsurf",
    version: env!("CARGO_PKG_VERSION"),
};

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
        Some(Command::Doctor) => {
            let env = |name: &str, default: &str| std::env::var(name).unwrap_or(default.into());
            std::process::exit(doctor::doctor(
                &env("LXSURF_MODEL", openrouter::chat::DEFAULT_MODEL),
                &env("LXSURF_DECISION_MODEL", system_one::DEFAULT_MODEL),
            ))
        }
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
                &format!("no guide named {name}; see `lxsurf skills list`"),
            ),
        },
    }
}

/// Where sessions are kept: `LXSURF_SESSIONS`, else `~/.lxsurf/sessions`.
fn sessions_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("LXSURF_SESSIONS").filter(|dir| !dir.is_empty()) {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .unwrap_or_default();
    PathBuf::from(home).join(".lxsurf/sessions")
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
    let schema = args.schema.as_deref().map(|arg| {
        start::read_schema(arg).unwrap_or_else(|message| quit(exit::ENVIRONMENT, &message))
    });
    let key = match args.dry_run {
        true => String::new(),
        false => match openrouter::auth::key(APP) {
            Some((key, _)) => key,
            None => quit(
                exit::ENVIRONMENT,
                "no OpenRouter key: set OPENROUTER_API_KEY or run `lxsurf login`",
            ),
        },
    };
    let headless = !args.show && (args.headless || start::headless_by_default());
    if !browser::has_display() {
        quit(
            exit::ENVIRONMENT,
            "no display for the browser window: run under `xvfb-run lxsurf …`",
        );
    }
    // A continued session resumes on the page its last goal left.
    let resume_at = resumed
        .as_ref()
        .map(|session| session.location.clone())
        .filter(|location| !location.is_empty());
    let timeout = Duration::from_secs(args.timeout);
    browser::host(headless, exit::INTERRUPTED, move |host| {
        let page = Page::new(host, headless);
        if args.dry_run {
            return dry_run(&args, &page, goal.as_deref().unwrap_or(""));
        }
        let client = OpenRouter::new(key, APP);
        let chat_model = ChatModel {
            client: &client,
            model: args.model.clone(),
        };
        let decider = SystemOne {
            client: &client,
            model: args.decision_model.clone(),
        };
        let mut options = run::Options {
            goal: goal.clone().unwrap_or_default(),
            chat_model: args.model.clone(),
            decision_model: args.decision_model.clone(),
            no_llm: args.no_llm,
            schema,
            expects: args.expects.clone(),
            max_turns: args.max_turns,
            max_steps: args.max_steps,
            json: args.json,
            check,
            trace: args.trace.clone(),
            screenshot: args.screenshot.clone(),
            resume_at,
            verbose: args.verbose,
        };
        let mut chat = Chat {
            page: &page,
            models: run::Models {
                chat: &chat_model,
                decider: &decider,
            },
            history: &history,
            session: resumed.unwrap_or_else(|| history.start()),
            conversation: None,
            deadline: Deadline::start(),
            timeout,
        };
        let mut code = match goal {
            Some(_) => chat.turn(&options),
            None => exit::OK,
        };
        // Only the first goal resumes; the next ones go on where the last one left the page.
        options.resume_at = None;
        if !args.keep_open {
            return code;
        }
        if !chat::interactive(args.json) {
            eprintln!("lxsurf: done (exit {code}); close the window to quit");
            loop {
                std::thread::park();
            }
        }
        // Follow-ups are plain goals: the expectations, the data and the files were the first's.
        options.expects.clear();
        options.schema = None;
        options.check = false;
        options.trace = None;
        options.screenshot = None;
        eprintln!("lxsurf: type the next goal, or /help (Ctrl-D to quit)");
        while let Some(next) = chat::next_line() {
            if chat::command(&next, &page) {
                continue;
            }
            options.goal = next;
            code = chat.turn(&options);
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

/// Open the page `goal` names (or a search page) and print what the decision model would read.
fn dry_run(args: &RunArgs, page: &Page, goal: &str) -> i32 {
    let url = url_in(goal).unwrap_or_else(|| SEARCH_PAGE.to_string());
    match page.open(&url, goal).and_then(|()| page.observe()) {
        Ok(view) if args.json => {
            println!("{}", serde_json::json!({"view": view}));
            exit::OK
        }
        Ok(view) => {
            print!("{}", report::view_table(&view));
            exit::OK
        }
        Err(error) => {
            eprintln!("lxsurf: {error}");
            exit::ENVIRONMENT
        }
    }
}

fn quit(code: i32, message: &str) -> ! {
    eprintln!("lxsurf: {message}");
    std::process::exit(code)
}
