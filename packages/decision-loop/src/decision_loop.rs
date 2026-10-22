//! The decision loop: a goal in, a tool picked from the toolbox and driven there by the decision
//! model alone, as far as it takes. A goal run stops on its own guards (the same action twice
//! without a change, a view seen again, a refused action, its share of actions); the loop resumes
//! it with everything done so far as its history, until the model says DONE or BLOCKED, the action
//! budget is spent, or runs keep stopping without moving the tool. A tool that is BLOCKED hands the
//! goal to another one that fits it, if the toolbox has one.

use std::collections::BTreeMap;

use serde_json::json;

use crate::goal::{GoalOutcome, GoalRun, Stop};
use agent::model::{DecideError, Decider, Question};

use crate::choice::valid_choice;
use crate::step::{GoalStep, Operation};
use crate::toolbox::{SharedTool, Toolbox};
use agent::tool::Tool;

/// Actions one goal run takes before the loop looks at how it stopped.
const SEGMENT: usize = 20;
/// Runs in a row that stopped without a step that changed the tool: the goal is stuck.
const MAX_STALLS: usize = 2;

pub struct DecisionLoop<'a> {
    /// The tools there are now; read each time a tool is picked.
    pub toolbox: &'a Toolbox,
    pub decider: &'a dyn Decider,
    pub on_step: &'a dyn Fn(&GoalStep),
    /// Actions for the whole goal.
    pub max_actions: usize,
}

/// How a goal ended, and with which tool.
pub struct LoopOutcome {
    pub tool: Option<String>,
    pub run: GoalOutcome,
}

fn actions(steps: &[GoalStep]) -> usize {
    steps
        .iter()
        .filter(|step| !matches!(step.operation, Operation::Wait | Operation::Done))
        .count()
}

impl DecisionLoop<'_> {
    pub fn run(&self, goal: &str, values: &[String]) -> LoopOutcome {
        let mut tried: Vec<String> = vec![];
        let mut last: Option<LoopOutcome> = None;
        let mut spent = 0;
        loop {
            let left: Vec<SharedTool> = self
                .toolbox
                .tools()
                .into_iter()
                .filter(|tool| !tried.iter().any(|name| name == tool.name()))
                .collect();
            let tool = match self.pick(goal, &left) {
                Ok(tool) => tool,
                // No other tool: the last one's outcome stands.
                Err(stop) => {
                    return last.unwrap_or(LoopOutcome {
                        tool: None,
                        run: GoalOutcome::stopped(stop),
                    });
                }
            };
            tried.push(tool.name().to_string());
            let name = Some(tool.name().to_string());
            if let Err(error) = tool.start(goal) {
                let stop = Stop::ActionFailed(format!("{} could not start: {error}", tool.name()));
                last = Some(LoopOutcome {
                    tool: name,
                    run: GoalOutcome::stopped(stop),
                });
                continue;
            }
            let budget = self.max_actions.saturating_sub(spent);
            let run = self.drive(tool.as_ref(), goal, values, budget);
            spent += actions(&run.steps);
            let blocked = matches!(run.stop, Stop::Blocked);
            last = Some(LoopOutcome { tool: name, run });
            if !blocked || spent >= self.max_actions {
                return last.unwrap();
            }
        }
    }

    /// The tool for `goal` among `tools`: the only one, or the one the model picks by what each
    /// is for.
    fn pick(&self, goal: &str, tools: &[SharedTool]) -> Result<SharedTool, Stop> {
        match tools {
            [] => return Err(Stop::Blocked),
            [only] => return Ok(only.clone()),
            _ => {}
        }
        let criteria = tools
            .iter()
            .map(|tool| (tool.name().to_string(), json!(tool.description())))
            .collect();
        let question = Question::Choice {
            instructions: json!(format!(
                "Goal: {goal}\n\nWhich tool can reach this goal? Choose the one whose purpose fits it."
            )),
            criteria,
        };
        let questions = BTreeMap::from([("tool".to_string(), question.clone())]);
        let decision = self
            .decider
            .decide(&json!({"goal": goal}), &questions)
            .map_err(|error| match error {
                DecideError::Unavailable(message) => Stop::Unreachable(message),
                DecideError::Invalid(message) => Stop::NoDecision(message),
            })?;
        let Question::Choice { criteria, .. } = &question else {
            unreachable!("built as a choice above")
        };
        let (name, _) = valid_choice(decision.answers.get("tool"), criteria)
            .ok_or_else(|| Stop::NoDecision("no offered tool was chosen".into()))?;
        Ok(tools
            .iter()
            .find(|tool| tool.name() == name)
            .unwrap()
            .clone())
    }

    fn drive(&self, tool: &dyn Tool, goal: &str, values: &[String], budget: usize) -> GoalOutcome {
        let mut steps = vec![];
        let (mut decisions, mut cost, mut millis, mut stalls) = (0, 0.0, 0, 0);
        loop {
            let left = budget.saturating_sub(actions(&steps));
            let before = steps.len();
            let mut outcome = GoalRun {
                tool,
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
            stalls = if moved { 0 } else { stalls + 1 };
            let again = match &outcome.stop {
                Stop::Done { .. } | Stop::Blocked | Stop::Unreachable(_) => false,
                // Its share of actions spent: a checkpoint, not a failure.
                Stop::Limit => left > SEGMENT,
                Stop::Repeated | Stop::Cycling | Stop::ActionFailed(_) | Stop::NoDecision(_) => {
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

#[cfg(test)]
#[path = "decision_loop_tests.rs"]
mod tests;
