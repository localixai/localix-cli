//! The chat `--keep-open` leaves in the terminal: each line typed is the next goal, run with the
//! same tools and in the same session. The time limit is armed per goal, so waiting for the person
//! to type never times out.

use std::io::{BufRead, IsTerminal, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use std::path::{Path, PathBuf};

use agent::history::{History, Session, Turn};
use agent::model::Decider;
use agent::tool::Tool;
use decision_loop::toolbox::{SharedTool, Toolbox};

use crate::exit;
use crate::report;
use crate::run::{self, Options};

/// `--timeout` for one goal at a time: exits the process when a goal runs past it.
#[derive(Clone)]
pub struct Deadline(Arc<Mutex<Option<Instant>>>);

impl Deadline {
    pub fn start() -> Self {
        let deadline = Self(Arc::new(Mutex::new(None)));
        let watched = deadline.0.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(250));
                if watched
                    .lock()
                    .unwrap()
                    .is_some_and(|at| Instant::now() >= at)
                {
                    eprintln!("decision-loop: timed out");
                    std::process::exit(exit::EXHAUSTED);
                }
            }
        });
        deadline
    }

    pub fn arm(&self, after: Duration) {
        *self.0.lock().unwrap() = Some(Instant::now() + after);
    }

    pub fn disarm(&self) {
        *self.0.lock().unwrap() = None;
    }
}

/// Whether a person is at the terminal to type the next goal.
pub fn interactive(json: bool) -> bool {
    !json && std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

/// The next line typed (a goal or a `/command`), or `None` at the end of input (Ctrl-D) or `/exit`.
pub fn next_line() -> Option<String> {
    let stdin = std::io::stdin();
    loop {
        eprint!("\n› ");
        let _ = std::io::stderr().flush();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).ok()? == 0 {
            eprintln!();
            return None;
        }
        match line.trim() {
            "" => continue,
            "/exit" | "/quit" => return None,
            goal => return Some(goal.to_string()),
        }
    }
}

/// One goal in the session: run it within the time limit, record and save it.
pub fn turn(
    options: &Options,
    toolbox: &Toolbox,
    model: &dyn Decider,
    history: &dyn History,
    session: &mut Session,
    deadline: &Deadline,
    timeout: Duration,
) -> i32 {
    deadline.arm(timeout);
    let finished = run::run(options, toolbox, model);
    deadline.disarm();
    session.record(
        Turn {
            goal: options.goal.clone(),
            tool: finished.tool,
            answer: finished.answer,
            status: finished.status.into(),
            at: session::now(),
        },
        finished.url.as_deref(),
    );
    match history.save(session) {
        Ok(()) if !options.json => eprintln!("  session  {}", session.id),
        Ok(()) => {}
        Err(error) => eprintln!("decision-loop: the session was not saved: {error}"),
    }
    finished.code
}

/// What the chat's `/commands` act on: the tool the last goal used, else the first one.
fn current(toolbox: &Toolbox, session: &Session) -> Option<SharedTool> {
    session
        .turns
        .last()
        .and_then(|turn| turn.tool.as_deref())
        .and_then(|name| toolbox.get(name))
        .or_else(|| toolbox.tools().into_iter().next())
}

/// Run a `/command` typed in the chat; `false` when `line` is not one.
pub fn command(line: &str, toolbox: &Toolbox, session: &Session) -> bool {
    let Some(rest) = line.strip_prefix('/') else {
        return false;
    };
    let (name, argument) = rest.split_once(' ').unwrap_or((rest, ""));
    match name {
        "tools" => {
            for tool in toolbox.tools() {
                eprintln!("{:<10} {}", tool.name(), tool.description());
            }
        }
        "snapshot" => match current(toolbox, session).map(|tool| tool.observe()) {
            Some(Ok(view)) => eprint!("{}", report::view_table(&view)),
            Some(Err(error)) => eprintln!("decision-loop: {error}"),
            None => eprintln!("decision-loop: no tool"),
        },
        "screenshot" => {
            let path = if argument.trim().is_empty() {
                PathBuf::from(format!("screenshot-{}.png", session::now()))
            } else {
                PathBuf::from(argument.trim())
            };
            match current(toolbox, session).map(|tool| save_capture(tool.as_ref(), &path)) {
                Some(Ok(saved)) => eprintln!("saved {}", saved.display()),
                Some(Err(error)) => eprintln!("decision-loop: {error}"),
                None => eprintln!("decision-loop: no tool"),
            }
        }
        _ => eprintln!(
            "commands: /snapshot (what the tool shows, by ref) · /screenshot [FILE] · /tools · /exit"
        ),
    }
    true
}

/// Save a picture of what `tool` shows to `path`; its absolute path.
pub fn save_capture(tool: &dyn Tool, path: &Path) -> Result<PathBuf, String> {
    let capture = tool
        .capture()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("{} takes no pictures", tool.name()))?;
    std::fs::write(path, capture.bytes).map_err(|error| error.to_string())?;
    Ok(std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()))
}
