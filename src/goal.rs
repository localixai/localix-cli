//! `goal`: the decision model drives the current page toward a short sub-goal — observe, one
//! decision request, one action, repeat, bounded. Ported from Localix's `browser-goal.ts`.
//!
//! It never decides success. It stops on DONE or BLOCKED, an unreadable answer, a failed action,
//! the step cap, or the same action twice without a visible change, and returns the trace and the
//! final page: the caller reads the page and judges.

use std::time::Duration;

use crate::browser::{Action, BrowserError, Driver, Observation};
use crate::decision::Decider;
use crate::goal_ask::{AskInput, GoalStep, Operation, Pick, ask};
pub use crate::step::Stop;

pub const MAX_ACTIONS: usize = 20;
pub const MAX_DECISIONS: usize = 40;
/// Below this probability a DONE is reported as unsure.
pub const DONE_CONFIDENCE: f64 = 0.6;
/// A WAIT looks again every `WAIT_POLL` until the page changes, for at most `WAIT_BUSY` while the
/// page says it is working and `WAIT_IDLE` otherwise.
const WAIT_POLL: Duration = Duration::from_millis(250);
const WAIT_BUSY: Duration = Duration::from_secs(5);
const WAIT_IDLE: Duration = Duration::from_millis(1500);
/// How long a finished goal waits for its page to stop changing before it reports it.
const SETTLE_MAX: Duration = Duration::from_secs(2);

pub struct GoalOutcome {
    pub steps: Vec<GoalStep>,
    pub stop: Stop,
    pub page: Option<Observation>,
    pub decisions: usize,
    pub cost: f64,
    pub decision_millis: u64,
}

pub struct GoalRun<'a> {
    pub driver: &'a dyn Driver,
    pub decider: &'a dyn Decider,
    /// Each executed step as it happens (the live trace).
    pub on_step: &'a dyn Fn(&GoalStep),
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

/// `https://host:port` of a URL, the boundary a goal run does not cross on its own.
fn origin(url: &str) -> &str {
    let after = url.find("://").map_or(0, |at| at + 3);
    let end = url[after..]
        .find(['/', '?', '#'])
        .map_or(url.len(), |at| after + at);
    &url[..end]
}

/// What the page looks like to the model, as one number: address, words and controls (by what they
/// are, not their refs, which change as a page re-renders).
fn fingerprint(page: &Observation) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    page.url.hash(&mut hasher);
    page.text.hash(&mut hasher);
    for element in &page.elements {
        element.describe().hash(&mut hasher);
    }
    hasher.finish()
}

/// A loop: the page came back, a third time, to a state it has already been in (open a menu, close
/// it, open it again). Repeating an action that keeps showing something new (a carousel's "next",
/// "load more") is progress, not a loop.
fn cycling(seen: &[u64], now: u64) -> bool {
    seen.iter().filter(|state| **state == now).count() >= 2
}

/// Whether the page differs in what the model can see: address, words or controls.
fn changed(before: &Observation, after: &Observation) -> bool {
    before.url != after.url
        || before.text != after.text
        || before.elements != after.elements
        || before.scroll.y != after.scroll.y
}

impl GoalRun<'_> {
    /// The page once it differs from `page`, or as it is after the longest wait: one decision per
    /// change instead of one per fixed pause.
    fn wait_for_change(&self, page: &Observation) -> Option<Observation> {
        let most = if page.busy { WAIT_BUSY } else { WAIT_IDLE };
        let deadline = std::time::Instant::now() + most;
        loop {
            std::thread::sleep(WAIT_POLL);
            let now = self.driver.observe().ok();
            let done = now.as_ref().is_some_and(|now| changed(page, now));
            if done || std::time::Instant::now() >= deadline {
                return now;
            }
        }
    }

    /// The page to hand back, once late content has arrived: a date picker fills in its prices,
    /// results finish rendering. Two identical looks in a row on a page that is not busy, for at
    /// most `SETTLE_MAX`; otherwise the planner would spend a turn finding the page half-loaded.
    fn settled(&self, last: Option<Observation>) -> Option<Observation> {
        let deadline = std::time::Instant::now() + SETTLE_MAX;
        let mut previous = last.map(Ok).unwrap_or_else(|| self.driver.observe()).ok()?;
        loop {
            std::thread::sleep(WAIT_POLL);
            let Ok(now) = self.driver.observe() else {
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
        let mut outcome = GoalOutcome {
            steps: vec![],
            stop: Stop::Limit,
            page: None,
            decisions: 0,
            cost: 0.0,
            decision_millis: 0,
        };
        let mut latest: Option<Observation> = None;
        let mut retried_stale = false;
        let mut seen: Vec<u64> = vec![];
        while outcome.decisions < MAX_DECISIONS {
            let actions = outcome
                .steps
                .iter()
                .filter(|step| step.operation != Operation::Wait)
                .count();
            if actions >= self.max_actions {
                break;
            }
            let page = match latest
                .take()
                .map(Ok)
                .unwrap_or_else(|| self.driver.observe())
            {
                Ok(page) => page,
                Err(error) => {
                    outcome.stop =
                        Stop::ActionFailed(format!("the page could not be read: {error}"));
                    break;
                }
            };
            let request = ask(&AskInput {
                goal,
                page: &page,
                values,
                history: &outcome.steps,
            });
            if std::env::var_os("LXSURF_DEBUG").is_some() {
                eprintln!("decision state: {}", request.state);
            }
            outcome.decisions += 1;
            let decision = match self.decider.decide(&request.state, &request.questions) {
                Ok(decision) => decision,
                Err(error) => {
                    outcome.stop = Stop::NoDecision(error.to_string());
                    outcome.page = Some(page);
                    break;
                }
            };
            outcome.cost += decision.cost;
            outcome.decision_millis += decision.millis;
            let pick = match request.read(&decision.answers) {
                Ok(pick) => pick,
                Err(error) => {
                    outcome.stop = Stop::NoDecision(error);
                    outcome.page = Some(page);
                    break;
                }
            };
            let element = pick
                .reference
                .as_ref()
                .and_then(|reference| page.elements.iter().find(|e| &e.reference == reference))
                .map(|element| element.describe());
            let mut step = GoalStep {
                operation: pick.operation,
                reference: pick.reference.clone(),
                element,
                value: pick.value.clone(),
                probability: pick.probability,
                changed: None,
            };
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
                    outcome.page = Some(page);
                    break;
                }
                Operation::Wait => {
                    let after = self.wait_for_change(&page);
                    step.changed = Some(after.as_ref().is_some_and(|after| changed(&page, after)));
                    (self.on_step)(&step);
                    outcome.steps.push(step);
                    latest = after;
                    continue;
                }
                _ => {}
            }
            let Some(action) = action_of(&pick) else {
                outcome.stop = Stop::NoDecision("a step without its value".into());
                outcome.page = Some(page);
                break;
            };
            let repeated = outcome.steps.last().is_some_and(|last| {
                last.operation == step.operation
                    && last.reference == step.reference
                    && last.value == step.value
                    && last.changed == Some(false)
            });
            match self.driver.act(&page.epoch, &action) {
                // A target gone since the snapshot (a popup still opening) is a stale read, not a
                // failed action: look again and decide again, once per step.
                Err(BrowserError::Stale(_)) if !retried_stale => {
                    retried_stale = true;
                    continue;
                }
                Err(error) => {
                    step.changed = Some(false);
                    (self.on_step)(&step);
                    outcome.steps.push(step);
                    outcome.stop = Stop::ActionFailed(error.to_string());
                    break;
                }
                Ok(()) => retried_stale = false,
            }
            let after = self.driver.observe();
            step.changed = Some(after.as_ref().map_or(true, |after| changed(&page, after)));
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
            // Another site is a new situation: hand it back to the planner to look at.
            if let Some(now) = latest
                .as_ref()
                .filter(|now| origin(&now.url) != origin(&page.url))
            {
                outcome.stop = Stop::Navigated(now.url.clone());
                break;
            }
        }
        let last = outcome.page.take().or(latest);
        outcome.page = self.settled(last);
        outcome
    }
}

#[cfg(test)]
#[path = "goal_tests.rs"]
mod tests;
