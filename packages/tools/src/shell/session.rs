//! The terminal sessions that stay open between calls, owned by the app: at most
//! [`MAX_SESSIONS`], each a program in its own [`Pty`], and none outlives the task that started
//! it. A call waits on a session until the text it waits for appears, the output settles, the
//! program exits, the person stops the task, or the call's time is up; the wait wakes on output,
//! not on a timer.

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use super::process::{Ending, Env};
use super::pty;
use super::running::Session;
use super::watch::{self, Watch};

pub const MAX_SESSIONS: usize = 4;
const DEFAULT_YIELD_MS: u64 = 2_000;
const NEEDLE_YIELD_MS: u64 = 15_000;
pub(super) const MIN_YIELD_MS: u64 = 100;
pub(super) const MAX_YIELD_MS: u64 = 60_000;
/// Characters of a command that name its session in reports and questions.
const SHOWN: usize = 60;

/// What a call waits for: `needle` in the new output, else the output to settle, for at most
/// `yield_ms`.
#[derive(Debug, Default, Clone, Copy)]
pub struct Wait<'a> {
    pub needle: Option<&'a str>,
    pub yield_ms: Option<u64>,
}

impl Wait<'_> {
    pub(super) fn limit(&self) -> Duration {
        let default = match self.needle {
            Some(_) => NEEDLE_YIELD_MS,
            None => DEFAULT_YIELD_MS,
        };
        let millis = self.yield_ms.unwrap_or(default);
        Duration::from_millis(millis.clamp(MIN_YIELD_MS, MAX_YIELD_MS))
    }
}

/// How a call's wait ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Until {
    Matched,
    Settled,
    Deadline,
    Exited(Ending),
    /// The call was stopped: its wait ended, and its session runs on.
    Stopped,
    /// Closed by the call; `killed` once everything it started is known to be gone.
    Closed {
        killed: bool,
    },
}

/// What a call on a session did.
#[derive(Debug)]
pub struct Turn {
    pub id: u32,
    /// The command, as short as its name in a report.
    pub command: String,
    pub until: Until,
    pub waited: Duration,
    /// How long the session has run.
    pub age: Duration,
    pub output: Option<String>,
    /// The bytes typed, and of how many, when the program stopped reading its input.
    pub stalled: Option<(usize, usize)>,
}

#[derive(Default)]
pub struct Sessions {
    inner: Mutex<Registry>,
    watch: Option<Watch>,
    shown: bool,
    /// Where ids come from when they are shared (every lane of the app, so a session's id names
    /// it in the one window too); else the registry counts its own.
    numbers: Option<Arc<AtomicU32>>,
}

#[derive(Default)]
struct Registry {
    /// The last id given: ids are never given twice.
    next: u32,
    live: Vec<Session>,
    /// Sessions a call holds (starting, or waited on): they count, and are not lent twice.
    busy: Vec<u32>,
}

impl Sessions {
    /// Sessions shown through `watch` once kept: all of them when `shown`, else when asked.
    pub fn new(watch: Watch, shown: bool) -> Self {
        Self {
            inner: Mutex::default(),
            watch: Some(watch),
            shown,
            numbers: None,
        }
    }

    /// Ids from `numbers`, shared with other registries: no two of them give the same id.
    pub fn numbered(mut self, numbers: Arc<AtomicU32>) -> Self {
        self.numbers = Some(numbers);
        self
    }

    /// Session `id`'s screen now, as a terminal shows it, or why there is none.
    pub fn screen(&self, id: u32) -> Result<String, String> {
        let registry = self.registry();
        let Some(session) = registry.live.iter().find(|session| session.id == id) else {
            return Err(not_running(id, &registry));
        };
        let stream = session.pty.shared.lock();
        Ok(stream.screen_now())
    }

    /// Room for one more session, or why there is none: checked before the person is asked.
    pub fn room(&self) -> Result<(), String> {
        room(&self.registry())
    }

    /// The command of the running session `id` as reports name it, or why there is none.
    pub fn command(&self, id: u32) -> Result<String, String> {
        let registry = self.registry();
        match registry.live.iter().find(|session| session.id == id) {
            Some(session) => Ok(session.command.clone()),
            None => Err(not_running(id, &registry)),
        }
    }

    /// The running sessions' ids and commands.
    pub fn listed(&self) -> Vec<(u32, String)> {
        let registry = self.registry();
        registry
            .live
            .iter()
            .map(|s| (s.id, s.command.clone()))
            .collect()
    }

    /// The running sessions, as `1 (`python3 -q`), 3 (`psql`)`, or `none`.
    pub fn running(&self) -> String {
        list(&self.registry().live)
    }

    /// Start `command` in `dir` in a terminal, and wait for it as `wait` says. A command that
    /// ends within the wait is not kept.
    pub fn start(
        &self,
        command: &str,
        dir: &Path,
        env: Env,
        wait: Wait,
        show: Option<bool>,
        stop: &(dyn Fn() -> bool + Sync),
    ) -> Result<Turn, String> {
        let id = {
            let mut registry = self.registry();
            room(&registry)?;
            registry.next += 1;
            let id = match &self.numbers {
                Some(numbers) => numbers.fetch_add(1, Ordering::SeqCst) + 1,
                None => registry.next,
            };
            registry.busy.push(id);
            id
        };
        let pty = pty::spawn(command, dir, env, wait.needle).map_err(|error| {
            self.release(id);
            format!("the shell could not start: {error}")
        })?;
        let owner = super::running::owner(self.numbers.as_ref());
        let session = Session::new(id, shown(command), pty, owner);
        Ok(self.go_on(session, wait, stop, None, show.unwrap_or(self.shown)))
    }

    /// Type `input` and press `keys` in session `id` (neither, to only read it), then wait.
    pub fn send(
        &self,
        id: u32,
        input: &[u8],
        keys: &[String],
        wait: Wait,
        show: Option<bool>,
        stop: &(dyn Fn() -> bool + Sync),
    ) -> Result<Turn, String> {
        let session = self.take(id)?;
        match show {
            Some(true) => self.show(&session),
            Some(false) => session.pty.dismiss(),
            None => {}
        }
        let typing = !input.is_empty() || !keys.is_empty();
        session.pty.shared.lock().expect(wait.needle, typing);
        let stalled = match typing {
            true => Some(session.pty.type_in(input, keys)).filter(|(sent, all)| sent < all),
            false => None,
        };
        Ok(self.go_on(session, wait, stop, stalled, false))
    }

    /// End session `id`, and everything it started.
    pub fn close(&self, id: u32) -> Result<Turn, String> {
        let mut session = self.take(id)?;
        let killed = session.end();
        self.release(id);
        Ok(session.turn(Until::Closed { killed }, Duration::ZERO, None))
    }

    /// End every session; how many there were.
    pub fn close_all(&self) -> usize {
        let live = std::mem::take(&mut self.registry().live);
        let count = live.len();
        for mut session in live {
            session.end();
        }
        count
    }

    /// Wait on `session`, then keep it while it runs, in a window from then on when `open`.
    fn go_on(
        &self,
        mut session: Session,
        wait: Wait,
        stop: &(dyn Fn() -> bool + Sync),
        stalled: Option<(usize, usize)>,
        open: bool,
    ) -> Turn {
        let begun = std::time::Instant::now();
        let until = session.wait(wait, stop);
        let turn = session.turn(until, begun.elapsed(), stalled);
        match until {
            // A stop ends the wait, not the program: a REPL or a program the person watches goes
            // on, until it is closed.
            Until::Exited(_) | Until::Closed { .. } => drop(session),
            Until::Matched | Until::Settled | Until::Deadline | Until::Stopped => {
                if open {
                    self.show(&session);
                }
                let mut registry = self.registry();
                registry.busy.retain(|&busy| busy != turn.id);
                registry.live.push(session);
                registry.live.sort_by_key(|session| session.id);
                return turn;
            }
        }
        self.release(turn.id);
        turn
    }

    /// Show `session` in a window, unless one is open; it opens with no lock held.
    fn show(&self, session: &Session) {
        let open = || session.pty.shared.lock().attached();
        let Some(watch) = self.watch.as_ref().filter(|_| !open()) else {
            return;
        };
        if let Some(painter) = watch(session.id, &watch::title(session.id, &session.command)) {
            let first = session.pty.shared.lock().attach(painter);
            session.pty.shared.tell(Some(first));
        }
    }

    fn take(&self, id: u32) -> Result<Session, String> {
        let mut registry = self.registry();
        match registry.live.iter().position(|session| session.id == id) {
            Some(at) => {
                registry.busy.push(id);
                Ok(registry.live.remove(at))
            }
            None => Err(not_running(id, &registry)),
        }
    }

    fn release(&self, id: u32) {
        self.registry().busy.retain(|&busy| busy != id);
    }

    fn registry(&self) -> MutexGuard<'_, Registry> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Drop for Sessions {
    fn drop(&mut self) {
        self.close_all();
    }
}

fn room(registry: &Registry) -> Result<(), String> {
    match registry.live.len() + registry.busy.len() < MAX_SESSIONS {
        true => Ok(()),
        false => Err(format!(
            "{MAX_SESSIONS} sessions are running: {}: close one first (session, close: true)",
            list(&registry.live)
        )),
    }
}

fn not_running(id: u32, registry: &Registry) -> String {
    match registry.busy.contains(&id) {
        true => {
            format!("session {id} is busy in another call: read it again after that call returns")
        }
        false => format!(
            "session {id} is not running: it ended, or was closed. Running: {}",
            list(&registry.live)
        ),
    }
}

fn list(live: &[Session]) -> String {
    let named: Vec<String> = live
        .iter()
        .map(|session| format!("{} (`{}`)", session.id, session.command))
        .collect();
    match named.is_empty() {
        true => "none".into(),
        false => named.join(", "),
    }
}

/// `command` on one line, at most [`SHOWN`] characters: a session's name, not its whole text.
fn shown(command: &str) -> String {
    let line = command.split_whitespace().collect::<Vec<_>>().join(" ");
    match line.char_indices().nth(SHOWN) {
        Some((at, _)) => format!("{}…", line.get(..at).unwrap_or_default()),
        None => line,
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
