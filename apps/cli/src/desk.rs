//! The desk: the lanes of a session at the terminal. Each lane is a whole session of its own
//! (its conversation, agents, jobs, shells and stop) on a thread of its own, over what they all
//! share (the workspace, the browser's page, the session store). The person opens one with `/new`
//! and the others go on in the background. The desk ends when the last lane does, and when one
//! panics the others are stopped, so a scope never waits for ever on a lane at its prompt.

mod ending;
#[cfg(test)]
mod test_support;

use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::Scope;
use std::time::Duration;

use contracts::history::History;

use self::ending::{AllLanes, Lanes, Leaving};
use crate::commands::repl;
use crate::compose::{Rig, Seat, Shared};
use crate::tui::Ui;
use crate::tui::event::Event;

/// How often the desk looks whether its lanes are gone while nothing is asked of it.
const POLL: Duration = Duration::from_millis(200);
/// Lanes open at most: opening another closes the oldest idle one.
pub(crate) const MAX_LANES: usize = 8;
/// Saved sessions the list shows at most.
const SAVED: usize = 20;

/// What the interface asks of the desk.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Request {
    /// Open a lane, or show it when it is open already.
    Open(Target),
    /// This directory's saved sessions, for the list.
    List,
    /// "Quit anyway": every lane is stopped, then the process ends.
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// A new session.
    New,
    /// A session saved before, by its id.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "the list of sessions will send it")
    )]
    Saved(String),
}

/// What a lane does once built: its session at the prompt.
pub(crate) type Work<'w> = Box<dyn for<'r, 's> FnOnce(&'r mut Rig<'s>) -> i32 + Send + 'w>;

/// Builds a lane over what is shared, from its seat, and runs its work in it.
pub(crate) type Start<'a> = dyn Fn(&Shared, Seat, Work) -> i32 + Sync + 'a;

/// The session in `ui`: the first lane from `first` (with `task` as its first message), then a
/// lane for each request, until none is left; its exit code.
pub(crate) fn run(
    shared: &Shared,
    ui: &Ui,
    requests: Receiver<Request>,
    first: Seat,
    task: Option<String>,
    start: &Start<'_>,
    quit: &(dyn Fn() + Sync),
) -> i32 {
    let lanes = Lanes::default();
    std::thread::scope(|scope| {
        let _all = AllLanes { lanes: &lanes, ui };
        let desk = Desk {
            shared,
            lanes: &lanes,
            ui,
            start,
        };
        desk.open(scope, first, task, true);
        while lanes.count() > 0 && !lanes.panicked.load(Ordering::SeqCst) {
            match requests.recv_timeout(POLL) {
                Ok(Request::Open(target)) => desk.target(scope, target),
                Ok(Request::List) => desk.list(),
                Ok(Request::Quit) => lanes.quit(ending::QUIT_WAIT, quit),
                Err(RecvTimeoutError::Timeout) => {}
                // No interface to ask: the lanes end by themselves.
                Err(RecvTimeoutError::Disconnected) => lanes.wait(POLL),
            }
        }
    });
    crate::exit::OK
}

/// What opening a lane needs.
struct Desk<'e> {
    shared: &'e Shared,
    lanes: &'e Lanes,
    ui: &'e Ui,
    start: &'e Start<'e>,
}

impl<'e> Desk<'e> {
    /// Open `target`; a session open already is shown, never opened twice.
    fn target<'s>(&self, scope: &'s Scope<'s, 'e>, target: Target) {
        let (shared, ui) = (self.shared, self.ui);
        let full = !matches!(&target, Target::Saved(id) if self.lanes.has(id));
        // The interface closed one to make room: it may still be ending.
        let until = std::time::Instant::now() + Duration::from_secs(1);
        while full && self.lanes.count() >= MAX_LANES && std::time::Instant::now() < until {
            self.lanes.wait(POLL);
        }
        if full && self.lanes.count() >= MAX_LANES {
            let said = format!("llx: {MAX_LANES} sessions are open: stop or quit one first");
            return ui.send(Event::Error(said));
        }
        match target {
            Target::New => {
                let seat = shared.seat(shared.history.start(), Some(ui.clone()));
                self.open(scope, seat, None, false);
            }
            Target::Saved(id) if self.lanes.has(&id) => ui.send(Event::Show(id)),
            Target::Saved(id) => match shared.history.load(Some(&id)) {
                Ok(session) => {
                    let mut seat = shared.seat(session, Some(ui.clone()));
                    let root = shared.workspace.root();
                    seat.context = crate::context::resumed(&shared.fresh, &seat.session, root);
                    self.open(scope, seat, None, false);
                }
                Err(why) => ui.send(Event::Error(format!("llx: {why}"))),
            },
        }
    }

    /// This directory's saved sessions for the list, newest first, without the agents'.
    fn list(&self) {
        let here = self.shared.workspace.root().display().to_string();
        let saved = crate::commands::sessions::here(self.shared.history.list(), &here);
        let rows = saved.iter().take(SAVED);
        let rows = rows.map(|session| {
            let (title, meta) = crate::commands::sessions::brief(session);
            protocol::Saved {
                id: session.id.clone(),
                title,
                meta,
            }
        });
        self.ui.send(Event::Saved(rows.collect()));
    }

    /// Start a lane on its own thread; one opened after the first is shown at once.
    fn open<'s>(&self, scope: &'s Scope<'s, 'e>, seat: Seat, task: Option<String>, first: bool) {
        let id = seat.session.id.clone();
        self.lanes.join(&id);
        // A resumed session is known by its title, else by its first task.
        let first_task = seat.session.turns.first().map(|turn| turn.goal.clone());
        let name = Some(seat.session.title.clone())
            .filter(|title| !title.is_empty())
            .or(first_task)
            .unwrap_or_else(|| seat.session.name.clone());
        self.ui.send(Event::LaneStarted {
            id: id.clone(),
            name,
        });
        if !first {
            self.ui.send(Event::Show(id.clone()));
        }
        let (shared, lanes, start) = (self.shared, self.lanes, self.start);
        // The lane's own interface is the desk's, tagged with its id, as `lane` tags it.
        let tagged = self.ui.for_lane(&id);
        scope.spawn(move || {
            let _leaving = Leaving { lanes, id };
            let work: Work = Box::new(move |rig| {
                // `--image` goes with the first lane's first task.
                if first {
                    rig.attached = rig.options.images.clone();
                }
                let ui = rig.ui.clone().unwrap_or(tagged);
                repl::run(rig, &ui, task.as_deref())
            });
            start(shared, seat, work)
        });
    }
}

#[cfg(test)]
#[path = "desk_tests.rs"]
mod tests;
