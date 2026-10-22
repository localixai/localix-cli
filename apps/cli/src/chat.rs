//! The chat `--keep-open` leaves in the terminal: each line typed is the next goal, run in the
//! same window, the same conversation and the same session. The time limit is armed per goal, so
//! waiting for the person to type never times out.

use std::io::{BufRead, IsTerminal, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use std::path::{Path, PathBuf};

use agent::conversation::Conversation;
use contracts::history::{History, Session, Turn};
use contracts::view::View;
use tools::browser::Page;

use crate::exit;
use crate::report;
use crate::run::{self, Models, Options};

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
                    eprintln!("lxsurf: timed out");
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

/// What one goal runs with, and what goes on from one goal to the next.
pub struct Chat<'a> {
    pub page: &'a Page,
    pub models: Models<'a>,
    pub history: &'a dyn History,
    pub session: Session,
    /// The agent's conversation so far: the next goal goes on from it.
    pub conversation: Option<Conversation>,
    pub deadline: Deadline,
    pub timeout: Duration,
}

impl Chat<'_> {
    /// One goal in the session: run it within the time limit, record and save it.
    pub fn turn(&mut self, options: &Options) -> i32 {
        self.deadline.arm(self.timeout);
        let finished = run::run(options, self.page, &self.models, &mut self.conversation);
        self.deadline.disarm();
        record(self.history, &mut self.session, options, finished)
    }
}

fn record(
    history: &dyn History,
    session: &mut Session,
    options: &Options,
    finished: run::Finished,
) -> i32 {
    session.record(
        Turn {
            goal: options.goal.clone(),
            answer: finished.answer,
            status: finished.status.into(),
            at: session::now(),
        },
        finished.url.as_deref(),
    );
    match history.save(session) {
        Ok(()) if !options.json => eprintln!("  session  {}", session.id),
        Ok(()) => {}
        Err(error) => eprintln!("lxsurf: the session was not saved: {error}"),
    }
    finished.code
}

/// Run a `/command` typed in the chat; `false` when `line` is not one.
pub fn command(line: &str, page: &Page) -> bool {
    let Some(rest) = line.strip_prefix('/') else {
        return false;
    };
    let (name, argument) = rest.split_once(' ').unwrap_or((rest, ""));
    match name {
        "snapshot" => match page.observe() {
            Ok(view) => eprint!("{}", report::view_table(&view)),
            Err(error) => eprintln!("lxsurf: {error}"),
        },
        "screenshot" => {
            let path = match argument.trim() {
                "" => PathBuf::from(format!("screenshot-{}.png", session::now())),
                given => PathBuf::from(given),
            };
            match save_capture(page, &path) {
                Ok(saved) => eprintln!("saved {}", saved.display()),
                Err(error) => eprintln!("lxsurf: {error}"),
            }
        }
        _ => eprintln!("commands: /snapshot (the page, by ref) · /screenshot [FILE] · /exit"),
    }
    true
}

/// Save a picture of what `view` shows to `path`; its absolute path.
pub fn save_capture(view: &dyn View, path: &Path) -> Result<PathBuf, String> {
    let capture = view
        .capture()
        .map_err(|error| error.to_string())?
        .ok_or("the view takes no pictures")?;
    std::fs::write(path, capture.bytes).map_err(|error| error.to_string())?;
    Ok(std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()))
}
