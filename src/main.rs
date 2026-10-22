//! lxsurf: tell it a goal, it opens a native browser window, surfs there and answers.

use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};

use localix_surf::browser::{self, Driver, Observation, Viewport, Window};
use localix_surf::{auth, decision, doctor, exit, planner, run, skills, start, structured};

#[derive(Parser)]
#[command(
    name = "lxsurf",
    version,
    about = "Localix Surf: a goal in, a native browser driven to it, an answer out"
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
    Check(RunArgs),
    /// Sign in with OpenRouter in the browser and save the key it issues.
    Login,
    /// Forget the saved key.
    Logout,
    /// List OpenRouter models: chat models for --model, or decision models.
    Models {
        #[arg(long)]
        decision: bool,
    },
    /// Check the key, both models and the WebView runtime.
    Doctor,
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
    /// What to do on the web, in plain words; `-` reads it from stdin.
    goal: Option<String>,
    /// Page to start on (default: a site the goal names, else a search page).
    #[arg(long)]
    url: Option<String>,
    /// A statement that must hold on the final page; repeatable. Any false one fails the run.
    #[arg(long = "expect", value_name = "STATEMENT")]
    expects: Vec<String>,
    /// Keep the window invisible. Default under CI or when not in a terminal; set
    /// LXSURF_HEADLESS=1 to make it the default everywhere.
    #[arg(long, env = "LXSURF_HEADLESS", value_parser = clap::builder::BoolishValueParser::new())]
    headless: bool,
    /// Page size: desktop (default), tablet, mobile, or WIDTHxHEIGHT. Tablet and mobile also
    /// present as iOS Safari so sites serve their mobile layout.
    #[arg(long, default_value = "desktop")]
    viewport: String,
    /// Show the window even where it would be invisible by default (wins over --headless).
    #[arg(long)]
    show: bool,
    /// Chat model that plans (OpenRouter id).
    #[arg(long, env = "LXSURF_MODEL", default_value = planner::DEFAULT_MODEL)]
    model: String,
    /// Decision model that clicks and types (OpenRouter System One id).
    #[arg(long, env = "LXSURF_DECISION_MODEL", default_value = decision::DEFAULT_MODEL)]
    decision_model: String,
    /// Only the decision model: values come from the goal, the answer is the final page.
    #[arg(long)]
    no_llm: bool,
    /// Planner turns before giving up.
    #[arg(long, default_value_t = 50)]
    max_turns: usize,
    /// Decision-model actions per sub-goal.
    #[arg(long, default_value_t = 20)]
    max_steps: usize,
    /// Give up after this many seconds.
    #[arg(long, default_value_t = 600)]
    timeout: u64,
    /// Print one JSON object on stdout (schema 1).
    #[arg(long)]
    json: bool,
    /// Return the result as JSON matching this JSON Schema (inline, or @file.json): stdout gets
    /// the JSON alone; with --json it is under "data".
    #[arg(long, value_name = "SCHEMA")]
    schema: Option<String>,
    /// Write the full trace as JSON to this file.
    #[arg(long, value_name = "FILE")]
    trace: Option<PathBuf>,
    /// Leave the window open after the run until it is closed.
    #[arg(long)]
    keep_open: bool,
    /// Open the page, print what the decision model would be offered, and exit.
    #[arg(long)]
    dry_run: bool,
    /// Trace on stderr even with --json.
    #[arg(short, long)]
    verbose: bool,
}

const START_PAGE: &str = "https://duckduckgo.com/";

fn main() {
    let cli = Cli::parse();
    match cli.command {
        None => surf(cli.run, false),
        Some(Command::Check(args)) => surf(args, true),
        Some(Command::Login) => match auth::login() {
            Ok(path) => println!("Signed in. The key is saved in {}", path.display()),
            Err(error) => quit(exit::ENVIRONMENT, &error),
        },
        Some(Command::Logout) => match auth::logout() {
            Ok(true) => println!("Signed out."),
            Ok(false) => println!("No saved key."),
            Err(error) => quit(exit::ENVIRONMENT, &error.to_string()),
        },
        Some(Command::Models { decision }) => std::process::exit(doctor::models(decision)),
        Some(Command::Doctor) => std::process::exit(doctor::doctor()),
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

fn surf(args: RunArgs, check: bool) -> ! {
    let goal = start::read_goal(args.goal.as_deref())
        .unwrap_or_else(|message| quit(exit::ENVIRONMENT, &message));
    let schema = args
        .schema
        .as_deref()
        .map(structured::parse)
        .transpose()
        .unwrap_or_else(|message| quit(exit::ENVIRONMENT, &message));
    let viewport =
        Viewport::parse(&args.viewport).unwrap_or_else(|message| quit(exit::ENVIRONMENT, &message));
    if schema.is_some() && args.no_llm {
        quit(
            exit::ENVIRONMENT,
            "--schema needs the chat model; drop --no-llm",
        );
    }
    let url = args
        .url
        .clone()
        .or_else(|| start::url_in(&goal))
        .unwrap_or_else(|| START_PAGE.into());
    let key = if args.dry_run {
        String::new()
    } else {
        let Some((key, _)) = auth::key() else {
            quit(
                exit::ENVIRONMENT,
                "no OpenRouter key: set OPENROUTER_API_KEY or run `lxsurf login`",
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
    if !start::has_display() {
        quit(
            exit::ENVIRONMENT,
            "no display for the browser window: run under `xvfb-run lxsurf …`",
        );
    }
    let timeout = Duration::from_secs(args.timeout);
    std::thread::spawn(move || {
        std::thread::sleep(timeout);
        quit(exit::EXHAUSTED, "timed out");
    });
    let window = Window {
        url,
        title: format!("Localix Surf — {goal}"),
        headless,
        viewport,
    };
    browser::run(window, move |driver| {
        if let Err(error) = driver.wait_loaded(Duration::from_secs(20)) {
            eprintln!("lxsurf: {error}");
        }
        let code = if args.dry_run {
            dry_run(&args, &driver)
        } else {
            let options = run::Options {
                goal,
                key,
                model: args.model.clone(),
                decision_model: args.decision_model.clone(),
                no_llm: args.no_llm,
                expects: args.expects.clone(),
                max_turns: args.max_turns,
                max_steps: args.max_steps,
                json: args.json,
                check,
                trace: args.trace.clone(),
                verbose: args.verbose,
                schema: schema.clone(),
            };
            run::run(&options, &driver)
        };
        if args.keep_open {
            eprintln!("lxsurf: done (exit {code}); close the window to quit");
            loop {
                std::thread::park();
            }
        }
        code
    })
}

/// Print the page's element table: what a decision model would be offered.
fn dry_run(args: &RunArgs, driver: &browser::WebviewDriver) -> i32 {
    let observation = match driver.observe() {
        Ok(observation) => observation,
        Err(error) => {
            eprintln!("lxsurf: {error}");
            return exit::ENVIRONMENT;
        }
    };
    let frames = if args.verbose {
        driver.frames().ok()
    } else {
        None
    };
    if args.json {
        let mut value = serde_json::to_value(&observation).unwrap_or_default();
        if let Some(frames) = frames {
            value["frames"] = frames;
        }
        println!("{value}");
    } else {
        print_table(&observation);
        if let Some(frames) = frames {
            eprintln!("rendering: {frames}");
        }
    }
    exit::OK
}

fn print_table(observation: &Observation) {
    println!("{} — {}", observation.title, observation.url);
    for element in &observation.elements {
        let scope = if element.dialog { " (dialog)" } else { "" };
        println!("[{}] {}{scope}", element.reference, element.describe());
    }
    if observation.omitted > 0 {
        println!("… {} more controls past the cap", observation.omitted);
    }
}

fn quit(code: i32, message: &str) -> ! {
    eprintln!("lxsurf: {message}");
    std::process::exit(code)
}
