//! What runs in the lanes, as their events say it: for the question asked before quitting, which
//! names what would stop. Only lanes are known here, by the desk's `LaneStarted`: an agent's
//! task is its lane's work.

use std::sync::{Mutex, MutexGuard};

use protocol::{Event, JobRow, Running};

#[derive(Default)]
struct Lane {
    id: String,
    title: String,
    busy: bool,
    jobs: Vec<JobRow>,
}

/// Every lane's state, the first opened first.
#[derive(Default)]
pub(crate) struct Work {
    lanes: Mutex<Vec<Lane>>,
}

impl Work {
    /// `event`, sent in `lane`'s name (`None` for the server's own).
    pub(crate) fn saw(&self, lane: Option<&str>, event: &Event) {
        let mut lanes = self.lock();
        if let Event::LaneStarted { id, name } = event {
            let (id, title) = (id.clone(), name.clone());
            lanes.retain(|known| known.id != id);
            return lanes.push(Lane {
                id,
                title,
                ..Lane::default()
            });
        }
        let Some(state) = lanes.iter_mut().find(|known| Some(&*known.id) == lane) else {
            return;
        };
        match event {
            Event::TaskStarted(_) => state.busy = true,
            Event::TaskEnded { .. } => state.busy = false,
            Event::Updated(update) => {
                if let Some(title) = &update.title {
                    state.title.clone_from(title);
                }
            }
            Event::Jobs(rows) => state.jobs.clone_from(rows),
            Event::Header(_)
            | Event::Note(_)
            | Event::Error(_)
            | Event::Mode(_)
            | Event::Call { .. }
            | Event::Result { .. }
            | Event::Step(_)
            | Event::Answer(_)
            | Event::Earlier(_)
            | Event::LaneStarted { .. }
            | Event::LaneEnded
            | Event::Saved(_)
            | Event::Show(_)
            | Event::Started { .. }
            | Event::Closed
            | Event::Spent(_)
            | Event::Total(_)
            | Event::AgentsCleared
            | Event::Prefill(_)
            | Event::Context(_)
            | Event::Queue { .. }
            | Event::Steered(_)
            | Event::Copy(_)
            | Event::Quit => {}
        }
    }

    /// Whether lane `lane` is open: started, and not ended.
    pub(crate) fn open(&self, lane: &str) -> bool {
        self.lock().iter().any(|known| known.id == lane)
    }

    /// The lane ended: nothing of it runs.
    pub(crate) fn ended(&self, lane: &str) {
        self.lock().retain(|known| known.id != lane);
    }

    /// The lanes working, by title, the terminal sessions' `shells`, then the lanes' monitors
    /// and schedules, by name.
    pub(crate) fn running(&self, shells: Vec<String>) -> Vec<Running> {
        let lanes = self.lock();
        let working = lanes.iter().filter(|lane| lane.busy);
        let working = working.map(|lane| ("session", lane.title.clone()));
        let working = working.chain(shells.into_iter().map(|shell| ("shell", shell)));
        let jobs = lanes.iter().flat_map(|lane| &lane.jobs).map(|job| {
            let kind = if job.until_at.is_some() {
                "monitor"
            } else {
                "schedule"
            };
            (kind, job.name.clone())
        });
        let all = working.chain(jobs);
        all.map(|(kind, name)| Running {
            kind: kind.into(),
            name,
        })
        .collect()
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Lane>> {
        crate::sync::lock(&self.lanes)
    }
}

#[cfg(test)]
#[path = "work_tests.rs"]
mod tests;
