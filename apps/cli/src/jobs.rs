//! `jobs`: work that goes on in the background and comes back to the conversation later. A monitor runs a command and gives each batch of
//! lines it prints to the main agent: at its next step while a task runs, else as a new task. A
//! schedule gives its prompt as a task of its own when due, between tasks, behind what the person
//! queued. Both live as long as the session (a monitor until its deadline); in `exec` a task waits
//! for its monitors, and schedules are refused: the process ends with the task.

pub(crate) mod cron;
mod monitor;
pub(crate) mod spec;
mod tool;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use contracts::gate::Gate;
use tools::workspace::Workspace;

pub(crate) use self::tool::JobsTool;
use crate::output::Output;
use protocol::{Event, JobRow};

/// What the jobs are built from.
pub(crate) struct Kit<'a> {
    pub workspace: &'a Workspace,
    pub gate: &'a dyn Gate,
    /// What commands never see, read as each monitor starts.
    pub hidden: &'a (dyn Fn() -> Vec<String> + Sync),
    pub output: &'a Output,
    /// A session: idle time to run what comes back in, and schedules.
    pub session: bool,
    /// The lane's queue: what the jobs give back is posted there.
    pub queue: &'a queue::Queue,
}

/// The jobs of a session, shared by the tool, the monitors' threads and the scheduler.
#[derive(Default)]
pub(crate) struct Jobs {
    state: Mutex<State>,
    /// The session is over: every job ends.
    over: AtomicBool,
    /// The session's id: its jobs' stops are known under it (every session has a `job-1`).
    owner: String,
    /// Wakes the scheduler: a schedule added or stopped, the session over.
    bell: Arc<Bell>,
}

/// A count rung with a notification, so a ring between a look and the wait is not missed.
#[derive(Default)]
struct Bell {
    rung: Mutex<u64>,
    rang: Condvar,
}

impl Bell {
    fn ring(&self) {
        let mut rung = self.rung.lock().unwrap_or_else(PoisonError::into_inner);
        *rung = rung.wrapping_add(1);
        self.rang.notify_all();
    }

    fn rung(&self) -> u64 {
        *self.rung.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Until it rings past `seen`, or `most` passes (forever for `None`).
    fn wait(&self, seen: u64, most: Option<Duration>) {
        let rung = self.rung.lock().unwrap_or_else(PoisonError::into_inner);
        let quiet = |now: &mut u64| *now == seen;
        match most {
            Some(most) => drop(self.rang.wait_timeout_while(rung, most, quiet)),
            None => drop(self.rang.wait_while(rung, quiet)),
        }
    }
}

/// The longest the scheduler waits for a schedule unwoken: the wall clock may jump (the machine
/// slept), and a schedule is due by it.
const LONGEST: Duration = Duration::from_secs(60);

#[derive(Default)]
struct State {
    jobs: Vec<Job>,
    made: u32,
}

struct Job {
    id: String,
    name: String,
    what: What,
    stop: Arc<AtomicBool>,
    /// Batches a monitor gave.
    events: usize,
    /// Its stop, known to the interface's keys while it is listed.
    _known: Option<crate::interrupt::Known>,
}

enum What {
    Monitor {
        command: String,
        until: Instant,
    },
    Schedule {
        prompt: String,
        cron: Option<cron::Cron>,
        /// Unix seconds.
        next: u64,
        expires: u64,
    },
}

impl Jobs {
    /// The jobs of session `owner`.
    pub(crate) fn of(owner: &str) -> Self {
        Self {
            owner: owner.to_string(),
            ..Self::default()
        }
    }

    /// Know `stop` as job `id`'s stop, under the session's id.
    fn known(&self, id: &str, stop: &Arc<AtomicBool>) -> crate::interrupt::Known {
        let key = crate::interrupt::job_key(&self.owner, id);
        let (stop, bell) = (Arc::clone(stop), Arc::clone(&self.bell));
        crate::interrupt::register(&key, move || {
            stop.store(true, Ordering::SeqCst);
            bell.ring();
            false
        })
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        crate::output::lock(&self.state)
    }

    /// The task ended: in `exec` its monitors end (what they gave back goes by its kind).
    pub(crate) fn end(&self, kit: &Kit) {
        if !kit.session {
            for job in &self.lock().jobs {
                job.stop.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Whether the task must wait: in `exec`, a monitor still runs.
    pub(crate) fn expecting(&self, kit: &Kit) -> bool {
        !kit.session
            && self
                .lock()
                .jobs
                .iter()
                .any(|job| matches!(job.what, What::Monitor { .. }))
    }

    /// The session is over: every job ends.
    pub(crate) fn close(&self) {
        self.over.store(true, Ordering::SeqCst);
        for job in &self.lock().jobs {
            job.stop.store(true, Ordering::SeqCst);
        }
        self.bell.ring();
    }

    /// The rows the interface lists, sent whenever they change.
    fn show(&self, kit: &Kit) {
        let Some(ui) = kit.output.ui() else {
            return;
        };
        let rows = self.lock().jobs.iter().map(row).collect();
        ui.send(Event::Jobs(rows));
    }

    /// The scheduler: the schedules that are due give their prompt. It sleeps until the soonest
    /// is due, or a schedule is added or stopped, or the session is over.
    pub(crate) fn tick(&self, kit: &Kit) {
        loop {
            let seen = self.bell.rung();
            if self.over.load(Ordering::SeqCst) {
                return;
            }
            let now = session::now();
            let (mut due, mut changed, mut soonest) = (vec![], false, None::<u64>);
            let mut state = self.lock();
            state.jobs.retain_mut(|job| {
                let What::Schedule {
                    prompt,
                    cron,
                    next,
                    expires,
                } = &mut job.what
                else {
                    return true;
                };
                if job.stop.load(Ordering::SeqCst) {
                    changed = true;
                    return false;
                }
                if now >= *next {
                    due.push(crate::arrivals::schedule(&job.name, prompt));
                    match cron.as_ref().and_then(|cron| cron.next(now)) {
                        Some(at) if at <= *expires => *next = at,
                        _ => return false,
                    }
                }
                soonest = Some(soonest.map_or(*next, |soonest| soonest.min(*next)));
                true
            });
            drop(state);
            changed |= !due.is_empty();
            for prompt in due {
                kit.queue.post(prompt);
            }
            if changed {
                self.show(kit);
            }
            let wait = soonest.map(|next| {
                let left = Duration::from_secs(next.saturating_sub(session::now()).max(1));
                left.min(LONGEST)
            });
            self.bell.wait(seen, wait);
        }
    }
}

/// A job as the model lists it.
fn line(job: &Job) -> String {
    match &job.what {
        What::Monitor { command, until } => format!(
            "- monitor `{}` ({}): `{command}`, {} batches so far, {} min left",
            job.name,
            job.id,
            job.events,
            until
                .saturating_duration_since(Instant::now())
                .as_secs()
                .div_ceil(60)
        ),
        What::Schedule {
            prompt, cron, next, ..
        } => format!(
            "- schedule `{}` ({}): {}, next at {}: {prompt}",
            job.name,
            job.id,
            cron.as_ref()
                .map_or_else(|| "once".into(), |cron| cron.text().to_string()),
            cron::clock(*next)
        ),
    }
}

/// When `until` comes, in milliseconds since the epoch: the wire carries no `Instant`.
fn epoch_ms(until: Instant) -> u64 {
    let left = until.saturating_duration_since(Instant::now());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    u64::try_from((now + left).as_millis()).unwrap_or(u64::MAX)
}

/// A job as the interface lists it.
fn row(job: &Job) -> JobRow {
    match &job.what {
        What::Monitor { command, until } => JobRow {
            id: job.id.clone(),
            name: job.name.clone(),
            detail: command.clone(),
            until_at: Some(epoch_ms(*until)),
            next: None,
            events: job.events,
            lane: String::new(),
        },
        What::Schedule {
            prompt, cron, next, ..
        } => JobRow {
            id: job.id.clone(),
            name: job.name.clone(),
            detail: match cron {
                Some(cron) => format!("{} · {prompt}", cron.text()),
                None => prompt.clone(),
            },
            until_at: None,
            next: Some(cron::clock(*next)),
            events: 0,
            lane: String::new(),
        },
    }
}

#[cfg(test)]
#[path = "jobs_tests.rs"]
mod tests;
