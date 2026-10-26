//! `goal`: the decision model drives the current view toward a short task — observe, one
//! decision request, one action, repeat, bounded.
//!
//! It never decides success. It stops on DONE or BLOCKED, an unreadable answer, a failed action,
//! the step cap, or the same action again where it gains nothing (no visible change, or a value
//! typed again into the field it was just typed into), and returns the trace and the
//! final view: the caller reads the view and judges.

use std::time::Duration;

use crate::ask::{AskInput, GoalStep, Operation, Pick, ask};
pub use crate::step::Stop;
use crate::step::{action_of, actions_of, changed, cycling, fingerprint};
use contracts::model::{DecideError, Decider};
use contracts::view::{Observation, View, ViewError};

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
    pub on_step: &'a (dyn Fn(&GoalStep) + Sync),
    /// Waits before the view is looked at again: a view has no change event, and the wait is
    /// the caller's, as time is (ARCHITECTURE.md C1, C2).
    pub pause: &'a (dyn Fn(Duration) + Sync),
    /// Actions this run may take (not counting the steps it resumes from).
    pub max_actions: usize,
}

impl GoalRun<'_> {
    /// The view once it differs from `view`, or as it is after the longest wait: one decision per
    /// change instead of one per fixed pause.
    fn wait_for_change(&self, view: &Observation, most: Option<Duration>) -> Option<Observation> {
        let most = most.unwrap_or(if view.busy { WAIT_BUSY } else { WAIT_IDLE });
        let deadline = std::time::Instant::now() + most;
        loop {
            (self.pause)(WAIT_POLL);
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
            (self.pause)(WAIT_POLL);
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
        let before = actions_of(&steps);
        let mut walk = Walk {
            outcome: GoalOutcome {
                steps,
                ..GoalOutcome::stopped(Stop::Limit)
            },
            latest: None,
            retried_stale: false,
            doubted: false,
            seen: vec![],
        };
        while walk.outcome.decisions < MAX_DECISIONS {
            if actions_of(&walk.outcome.steps) - before >= self.max_actions {
                break;
            }
            if self.step(&mut walk, goal, values) == Flow::End {
                break;
            }
        }
        let last = walk.outcome.view.take().or(walk.latest);
        walk.outcome.view = self.settled(last);
        walk.outcome
    }

    /// One decision and what it leads to.
    fn step(&self, walk: &mut Walk, goal: &str, values: &[String]) -> Flow {
        let view = match walk
            .latest
            .take()
            .map(Ok)
            .unwrap_or_else(|| self.view.observe())
        {
            Ok(view) => view,
            Err(error) => {
                return walk.end(Stop::ActionFailed(format!(
                    "the view could not be read: {error}"
                )));
            }
        };
        let pick = match self.decide(walk, goal, values, &view) {
            Ok(pick) => pick,
            Err(stop) => {
                walk.outcome.view = Some(view);
                return walk.end(stop);
            }
        };
        let element = pick
            .reference
            .as_ref()
            .and_then(|reference| view.elements.iter().find(|e| &e.reference == reference))
            .map(|element| element.describe());
        let step = GoalStep {
            operation: pick.operation,
            reference: pick.reference.clone(),
            element,
            value: pick.value.clone(),
            probability: pick.probability,
            changed: None,
            failed: None,
            new_view: false,
        };
        self.carry_out(walk, &pick, step, view)
    }

    /// The pick the decision model makes on `view`, or why there is none.
    fn decide(
        &self,
        walk: &mut Walk,
        goal: &str,
        values: &[String],
        view: &Observation,
    ) -> Result<Pick, Stop> {
        let request = ask(&AskInput {
            goal,
            conventions: self.view.conventions(),
            view,
            values,
            history: &walk.outcome.steps,
        });
        walk.outcome.decisions += 1;
        let decision = self
            .decider
            .decide(&request.state, &request.questions)
            .map_err(|error| match error {
                DecideError::Unavailable(message) => Stop::Unreachable(message),
                DecideError::Invalid(message) => Stop::NoDecision(message),
            })?;
        walk.outcome.cost += decision.cost;
        walk.outcome.decision_millis += decision.millis;
        request.read(&decision.answers).map_err(Stop::NoDecision)
    }

    /// Doubt, end, wait or act, as `pick` says.
    fn carry_out(
        &self,
        walk: &mut Walk,
        pick: &Pick,
        mut step: GoalStep,
        view: Observation,
    ) -> Flow {
        // An unsure DONE, or one on a view still fetching, is often a view still arriving
        // (results being filled in): if the view changes, decide again on what arrived. A view
        // that stays as it is keeps the DONE: asking again there only invites one step too
        // many (a fourth quiz question for "three").
        if pick.operation == Operation::Done
            && (pick.probability < DONE_CONFIDENCE || view.busy)
            && !walk.doubted
        {
            walk.doubted = true;
            let after = self.wait_for_change(&view, None);
            if let Some(after) = after.filter(|after| changed(&view, after)) {
                (self.on_step)(&step);
                // The next decision reads it as a wait: what it was, whatever the model meant.
                walk.outcome.steps.push(GoalStep {
                    operation: Operation::Wait,
                    changed: Some(true),
                    ..step
                });
                walk.latest = Some(after);
                return Flow::Next;
            }
        }
        match pick.operation {
            Operation::Done | Operation::Blocked => {
                (self.on_step)(&step);
                walk.outcome.steps.push(step);
                walk.outcome.view = Some(view);
                walk.end(if pick.operation == Operation::Done {
                    Stop::Done {
                        probability: pick.probability,
                    }
                } else {
                    Stop::Blocked
                })
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
                walk.outcome.steps.push(step);
                walk.latest = after;
                Flow::Next
            }
            Operation::Click
            | Operation::TypeText
            | Operation::Select
            | Operation::PressEnter
            | Operation::ScrollDown
            | Operation::ScrollUp => self.act(walk, pick, step, view),
        }
    }

    /// Take the pick's action on `view`, and judge what it changed.
    fn act(&self, walk: &mut Walk, pick: &Pick, mut step: GoalStep, view: Observation) -> Flow {
        let Some(action) = action_of(pick) else {
            walk.outcome.view = Some(view);
            return walk.end(Stop::NoDecision("a step without its value".into()));
        };
        let same = walk.outcome.steps.last().filter(|last| {
            last.operation == step.operation
                && last.reference == step.reference
                && last.value == step.value
        });
        // The same value typed into the same field right after typing it is never progress: a
        // form's field holds it already, a terminal's line gets it twice. The run stops before
        // typing it again (a goal its values cannot carry out typed one forever).
        if same.is_some() && step.operation == Operation::TypeText {
            walk.outcome.view = Some(view);
            return walk.end(Stop::Repeated);
        }
        let repeated = same.is_some_and(|last| last.changed == Some(false));
        match self.view.act(&view.epoch, &action) {
            // A target gone since the snapshot (a popup still opening) is a stale read, not a
            // failed action: look again and decide again, once per step.
            Err(ViewError::Stale(_)) if !walk.retried_stale => {
                walk.retried_stale = true;
                return Flow::Next;
            }
            // Something came over the view since the snapshot (a dialog that appears late):
            // the next look offers it. Kept in the history, so the model knows why.
            Err(ViewError::Refused(error))
                if error.starts_with("covered") && !walk.retried_stale =>
            {
                walk.retried_stale = true;
                step.changed = Some(false);
                step.failed = Some(error);
                (self.on_step)(&step);
                walk.outcome.steps.push(step);
                return Flow::Next;
            }
            Err(error) => {
                step.changed = Some(false);
                step.failed = Some(error.to_string());
                (self.on_step)(&step);
                walk.outcome.steps.push(step);
                return walk.end(Stop::ActionFailed(error.to_string()));
            }
            Ok(()) => walk.retried_stale = false,
        }
        let after = self.view.observe();
        step.changed = Some(after.as_ref().map_or(true, |after| changed(&view, after)));
        step.new_view = after.as_ref().is_ok_and(|after| after.epoch != view.epoch);
        (self.on_step)(&step);
        let unchanged = step.changed == Some(false);
        walk.outcome.steps.push(step);
        walk.latest = after.ok();
        if repeated && unchanged {
            return walk.end(Stop::Repeated);
        }
        if let Some(now) = walk.latest.as_ref().map(fingerprint) {
            if cycling(&walk.seen, now) {
                return walk.end(Stop::Cycling);
            }
            walk.seen.push(now);
        }
        Flow::Next
    }
}

/// A run's state from one decision to the next.
struct Walk {
    outcome: GoalOutcome,
    latest: Option<Observation>,
    retried_stale: bool,
    doubted: bool,
    seen: Vec<u64>,
}

impl Walk {
    fn end(&mut self, stop: Stop) -> Flow {
        self.outcome.stop = stop;
        Flow::End
    }
}

/// Whether a run goes on to its next decision.
#[derive(PartialEq, Eq)]
enum Flow {
    Next,
    End,
}

#[cfg(test)]
#[path = "goal_tests.rs"]
mod tests;
