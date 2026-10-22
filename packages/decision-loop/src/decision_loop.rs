//! The decision loop: the hands that carry a task out on a view with a decision model alone. A
//! goal run stops on its own guards (the same action twice without a change, a view seen again, a
//! refused action, its share of actions); the loop resumes it with everything done so far as its
//! history, until the model says DONE or BLOCKED, the action budget is spent, or runs keep
//! stopping without changing the view.

use contracts::hands::{Hands, Outcome, Report, Task};
use contracts::model::Decider;
use contracts::view::View;

use crate::goal::{GoalOutcome, GoalRun, Stop};
use crate::step::{GoalStep, Operation};

/// Actions one goal run takes before the loop looks at how it stopped.
const SEGMENT: usize = 20;
/// Runs in a row that stopped without a step that changed the view: the task is stuck.
const MAX_STALLS: usize = 2;

pub struct DecisionLoop<'a> {
    pub decider: &'a dyn Decider,
    pub on_step: &'a dyn Fn(&GoalStep),
    /// Actions for one task.
    pub max_actions: usize,
}

fn actions(steps: &[GoalStep]) -> usize {
    steps
        .iter()
        .filter(|step| !matches!(step.operation, Operation::Wait | Operation::Done))
        .count()
}

impl DecisionLoop<'_> {
    /// Drive `view` toward `goal`, typing only `values`.
    pub fn run(&self, view: &dyn View, goal: &str, values: &[String]) -> GoalOutcome {
        let mut steps = vec![];
        let (mut decisions, mut cost, mut millis, mut stalls) = (0, 0.0, 0, 0);
        loop {
            let left = self.max_actions.saturating_sub(actions(&steps));
            let before = steps.len();
            let mut outcome = GoalRun {
                view,
                decider: self.decider,
                on_step: self.on_step,
                max_actions: SEGMENT.min(left),
            }
            .resume(goal, values, steps);
            decisions += outcome.decisions;
            cost += outcome.cost;
            millis += outcome.decision_millis;
            let moved = outcome.steps[before..]
                .iter()
                .any(|step| step.changed == Some(true) && step.failed.is_none());
            let again = match &outcome.stop {
                Stop::Done { .. } | Stop::Blocked | Stop::Unreachable(_) => false,
                // Its share of actions spent: a checkpoint, not a failure.
                Stop::Limit => left > SEGMENT,
                // Going round in circles changes the view at every step without getting anywhere:
                // a stall however much moved.
                Stop::Cycling => {
                    stalls += 1;
                    stalls < MAX_STALLS && left > 0
                }
                Stop::Repeated | Stop::ActionFailed(_) | Stop::NoDecision(_) => {
                    stalls = if moved { 0 } else { stalls + 1 };
                    stalls < MAX_STALLS && left > 0
                }
            };
            if !again {
                outcome.decisions = decisions;
                outcome.cost = cost;
                outcome.decision_millis = millis;
                return outcome;
            }
            steps = outcome.steps;
        }
    }
}

impl Hands for DecisionLoop<'_> {
    fn carry_out(&self, view: &dyn View, task: Task) -> Report {
        let run = self.run(view, task.goal, task.values);
        let outcome = match &run.stop {
            Stop::Done { probability } => Outcome::Done {
                probability: *probability,
            },
            Stop::Blocked => Outcome::Blocked,
            Stop::Limit => Outcome::Exhausted,
            Stop::Unreachable(error) => Outcome::Unreachable(error.clone()),
            other => Outcome::Stopped(other.to_string()),
        };
        Report {
            steps: run.steps.iter().map(GoalStep::trace_line).collect(),
            outcome,
            view: run.view,
        }
    }
}

#[cfg(test)]
#[path = "decision_loop_tests.rs"]
mod tests;
