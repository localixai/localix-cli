//! `goal`: the decision model drives the current view toward a short task — observe, one
//! decision request, one action, repeat, bounded.
//!
//! It never decides success. It stops on DONE or BLOCKED, an unreadable answer, a failed action,
//! the step cap, or the same action twice without a visible change, and returns the trace and the
//! final view: the caller reads the view and judges.

use std::time::Duration;

use crate::ask::{AskInput, GoalStep, Operation, Pick, ask};
pub use crate::step::Stop;
use contracts::model::{DecideError, Decider};
use contracts::view::{Action, Observation, View, ViewError};

pub const MAX_DECISIONS: usize = 40;
/// Below this probability a DONE is reported as unsure.
pub const DONE_CONFIDENCE: f64 = 0.6;
/// A WAIT looks again every `WAIT_POLL` until the view changes, for at most `WAIT_BUSY` while the
/// view says it is working and `WAIT_IDLE` otherwise.
const WAIT_POLL: Duration = Duration::from_millis(250);
const WAIT_BUSY: Duration = Duration::from_secs(5);
const WAIT_IDLE: Duration = Duration::from_millis(1500);
/// How long a finished goal waits for its view to stop changing before it reports it.
const SETTLE_MAX: Duration = Duration::from_secs(2);

pub struct GoalOutcome {
    pub steps: Vec<GoalStep>,
    pub stop: Stop,
    pub view: Option<Observation>,
    pub decisions: usize,
    pub cost: f64,
    pub decision_millis: u64,
}

impl GoalOutcome {
    /// A goal that ended before any step.
    pub fn stopped(stop: Stop) -> Self {
        Self {
            steps: vec![],
            stop,
            view: None,
            decisions: 0,
            cost: 0.0,
            decision_millis: 0,
        }
    }
}

pub struct GoalRun<'a> {
    pub view: &'a dyn View,
    pub decider: &'a dyn Decider,
    /// Each executed step as it happens (the live trace).
    pub on_step: &'a dyn Fn(&GoalStep),
    /// Actions this run may take (not counting the steps it resumes from).
    pub max_actions: usize,
}

fn action_of(pick: &Pick) -> Option<Action> {
    let reference = || pick.reference.clone().unwrap_or_default();
    Some(match pick.operation {
        Operation::Click => Action::Click {
            reference: reference(),
        },
        Operation::TypeText => Action::Type {
            reference: reference(),
            text: pick.value.clone()?,
        },
        Operation::Select => Action::Select {
            reference: reference(),
            value: pick.value.clone()?,
        },
        Operation::PressEnter => Action::PressEnter { reference: None },
        Operation::ScrollDown => Action::ScrollDown,
        Operation::ScrollUp => Action::ScrollUp,
        Operation::Wait | Operation::Done | Operation::Blocked => return None,
    })
}

/// What the view looks like to the model, as one number: address, words and controls (by what they
/// are, not their refs, which change as a view re-renders).
fn fingerprint(view: &Observation) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    view.location.hash(&mut hasher);
    view.text.hash(&mut hasher);
    for element in &view.elements {
        element.describe().hash(&mut hasher);
    }
    hasher.finish()
}

/// A loop: the view came back, a third time, to a state it has already been in (open a menu, close
/// it, open it again). Repeating an action that keeps showing something new (a carousel's "next",
/// "load more") is progress, not a loop.
fn cycling(seen: &[u64], now: u64) -> bool {
    seen.iter().filter(|state| **state == now).count() >= 2
}

/// Whether the view differs in what the model can see: address, words or controls.
fn changed(before: &Observation, after: &Observation) -> bool {
    before.location != after.location
        || before.text != after.text
        || before.elements != after.elements
        || before.scroll != after.scroll
}

impl GoalRun<'_> {
    /// The view once it differs from `view`, or as it is after the longest wait: one decision per
    /// change instead of one per fixed pause.
    fn wait_for_change(&self, view: &Observation, most: Option<Duration>) -> Option<Observation> {
        let most = most.unwrap_or(if view.busy { WAIT_BUSY } else { WAIT_IDLE });
        let deadline = std::time::Instant::now() + most;
        loop {
            std::thread::sleep(WAIT_POLL);
            let now = self.view.observe().ok();
            let done = now.as_ref().is_some_and(|now| changed(view, now));
            if done || std::time::Instant::now() >= deadline {
                return now;
            }
        }
    }

    /// The view to hand back, once late content has arrived (prices fill in, a list finishes
    /// rendering): two identical looks in a row on a view that is not busy, for at most
    /// `SETTLE_MAX`, so the caller does not judge a view half-loaded.
    fn settled(&self, last: Option<Observation>) -> Option<Observation> {
        let deadline = std::time::Instant::now() + SETTLE_MAX;
        let mut previous = last.map(Ok).unwrap_or_else(|| self.view.observe()).ok()?;
        loop {
            std::thread::sleep(WAIT_POLL);
            let Ok(now) = self.view.observe() else {
                return Some(previous);
            };
            let quiet = fingerprint(&now) == fingerprint(&previous) && !now.busy;
            if quiet || std::time::Instant::now() >= deadline {
                return Some(now);
            }
            previous = now;
        }
    }

    pub fn run(&self, goal: &str, values: &[String]) -> GoalOutcome {
        self.resume(goal, values, vec![])
    }

    /// Carry on toward `goal` after the steps already taken: the model reads them as its history.
    pub fn resume(&self, goal: &str, values: &[String], steps: Vec<GoalStep>) -> GoalOutcome {
        let actions_of = |steps: &[GoalStep]| {
            steps
                .iter()
                .filter(|step| step.operation != Operation::Wait)
                .count()
        };
        let before = actions_of(&steps);
        let mut outcome = GoalOutcome {
            steps,
            ..GoalOutcome::stopped(Stop::Limit)
        };
        let mut latest: Option<Observation> = None;
        let mut retried_stale = false;
        let mut doubted = false;
        let mut seen: Vec<u64> = vec![];
        while outcome.decisions < MAX_DECISIONS {
            if actions_of(&outcome.steps) - before >= self.max_actions {
                break;
            }
            let view = match latest.take().map(Ok).unwrap_or_else(|| self.view.observe()) {
                Ok(view) => view,
                Err(error) => {
                    outcome.stop =
                        Stop::ActionFailed(format!("the view could not be read: {error}"));
                    break;
                }
            };
            let request = ask(&AskInput {
                goal,
                conventions: self.view.conventions(),
                view: &view,
                values,
                history: &outcome.steps,
            });
            if std::env::var_os("DECISION_LOOP_DEBUG").is_some() {
                eprintln!("decision state: {}", request.state);
            }
            outcome.decisions += 1;
            let decision = match self.decider.decide(&request.state, &request.questions) {
                Ok(decision) => decision,
                Err(error) => {
                    outcome.stop = match error {
                        DecideError::Unavailable(message) => Stop::Unreachable(message),
                        DecideError::Invalid(message) => Stop::NoDecision(message),
                    };
                    outcome.view = Some(view);
                    break;
                }
            };
            if std::env::var_os("DECISION_LOOP_DEBUG").is_some() {
                eprintln!("decision answers: {:?}", decision.answers);
            }
            outcome.cost += decision.cost;
            outcome.decision_millis += decision.millis;
            let pick = match request.read(&decision.answers) {
                Ok(pick) => pick,
                Err(error) => {
                    outcome.stop = Stop::NoDecision(error);
                    outcome.view = Some(view);
                    break;
                }
            };
            let element = pick
                .reference
                .as_ref()
                .and_then(|reference| view.elements.iter().find(|e| &e.reference == reference))
                .map(|element| element.describe());
            let mut step = GoalStep {
                operation: pick.operation,
                reference: pick.reference.clone(),
                element,
                value: pick.value.clone(),
                probability: pick.probability,
                changed: None,
                failed: None,
                new_view: false,
            };
            // An unsure DONE, or one on a view still fetching, is often a view still arriving
            // (results being filled in): if the view changes, decide again on what arrived. A view
            // that stays as it is keeps the DONE: asking again there only invites one step too
            // many (a fourth quiz question for "three").
            if pick.operation == Operation::Done
                && (pick.probability < DONE_CONFIDENCE || view.busy)
                && !doubted
            {
                doubted = true;
                let after = self.wait_for_change(&view, None);
                if let Some(after) = after.filter(|after| changed(&view, after)) {
                    (self.on_step)(&step);
                    // The next decision reads it as a wait: what it was, whatever the model meant.
                    outcome.steps.push(GoalStep {
                        operation: Operation::Wait,
                        changed: Some(true),
                        ..step
                    });
                    latest = Some(after);
                    continue;
                }
            }
            match pick.operation {
                Operation::Done | Operation::Blocked => {
                    (self.on_step)(&step);
                    outcome.steps.push(step);
                    outcome.stop = if pick.operation == Operation::Done {
                        Stop::Done {
                            probability: pick.probability,
                        }
                    } else {
                        Stop::Blocked
                    };
                    outcome.view = Some(view);
                    break;
                }
                Operation::Wait => {
                    // As long as the model chose, and no longer than what changes the view takes.
                    let most = pick
                        .value
                        .as_deref()
                        .and_then(|seconds| seconds.parse().ok())
                        .map(Duration::from_secs);
                    let after = self.wait_for_change(&view, most);
                    step.changed = Some(after.as_ref().is_some_and(|after| changed(&view, after)));
                    (self.on_step)(&step);
                    outcome.steps.push(step);
                    latest = after;
                    continue;
                }
                _ => {}
            }
            let Some(action) = action_of(&pick) else {
                outcome.stop = Stop::NoDecision("a step without its value".into());
                outcome.view = Some(view);
                break;
            };
            let repeated = outcome.steps.last().is_some_and(|last| {
                last.operation == step.operation
                    && last.reference == step.reference
                    && last.value == step.value
                    && last.changed == Some(false)
            });
            match self.view.act(&view.epoch, &action) {
                // A target gone since the snapshot (a popup still opening) is a stale read, not a
                // failed action: look again and decide again, once per step.
                Err(ViewError::Stale(_)) if !retried_stale => {
                    retried_stale = true;
                    continue;
                }
                // Something came over the view since the snapshot (a dialog that appears late):
                // the next look offers it. Kept in the history, so the model knows why.
                Err(ViewError::Refused(error))
                    if error.starts_with("covered") && !retried_stale =>
                {
                    retried_stale = true;
                    step.changed = Some(false);
                    step.failed = Some(error);
                    (self.on_step)(&step);
                    outcome.steps.push(step);
                    continue;
                }
                Err(error) => {
                    step.changed = Some(false);
                    step.failed = Some(error.to_string());
                    (self.on_step)(&step);
                    outcome.steps.push(step);
                    outcome.stop = Stop::ActionFailed(error.to_string());
                    break;
                }
                Ok(()) => retried_stale = false,
            }
            let after = self.view.observe();
            step.changed = Some(after.as_ref().map_or(true, |after| changed(&view, after)));
            step.new_view = after.as_ref().is_ok_and(|after| after.epoch != view.epoch);
            (self.on_step)(&step);
            let unchanged = step.changed == Some(false);
            outcome.steps.push(step);
            latest = after.ok();
            if repeated && unchanged {
                outcome.stop = Stop::Repeated;
                break;
            }
            if let Some(now) = latest.as_ref().map(fingerprint) {
                if cycling(&seen, now) {
                    outcome.stop = Stop::Cycling;
                    break;
                }
                seen.push(now);
            }
        }
        let last = outcome.view.take().or(latest);
        outcome.view = self.settled(last);
        outcome
    }
}

#[cfg(test)]
#[path = "goal_tests.rs"]
mod tests;
