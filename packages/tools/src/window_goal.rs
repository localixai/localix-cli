//! A `goal` on a tab: the kind's own fields first, the goal handed to the helper with the tab's
//! view once the gate allows it, then what the helper did and the snapshot after.

use contracts::hands::{Outcome, Report, Task};
use contracts::tool::{ToolError, ToolOutput};
use serde_json::{Value, json};

use super::{WindowTool, failed};

impl WindowTool<'_> {
    /// A goal for the helper on the tab's view first, then the kind's own fields; the snapshot
    /// after.
    pub(super) fn interact(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        let (tab, kind, id) = self.tab(arguments.get("tab").and_then(Value::as_str))?;
        let goal = (arguments.get("goal").and_then(Value::as_str))
            .map(str::trim)
            .filter(|goal| !goal.is_empty());
        let Some(goal) = goal else {
            let done = kind.interact(id, arguments)?;
            return Ok(ToolOutput {
                report: format!("{tab}: {}", done.report),
                ..done
            });
        };
        let values: Vec<String> = (arguments.get("values").and_then(Value::as_array))
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        let hands = self.hands.ok_or_else(|| {
            failed("there is no helper here (no decision model): act with the tab's own fields")
        })?;
        // The kind's own fields first (an address to load), so the goal acts on what they leave.
        let own = kind
            .fields()
            .iter()
            .any(|(name, _)| arguments.get(*name).is_some());
        let first = match own {
            true => Some(kind.interact(id, arguments)?),
            false => None,
        };
        let mut summary = format!("goal on {tab}: {}", goal.replace('\n', " \u{23ce} "));
        if !values.is_empty() {
            let quoted: Vec<String> = values.iter().map(|v| format!("\"{v}\"")).collect();
            summary.push_str(&format!(" (may type {})", quoted.join(", ")));
        }
        crate::ask(self.gate, kind.name(), &summary)?;
        let done = {
            let view = kind.view(id)?;
            hands.carry_out(
                view.as_ref(),
                Task {
                    goal,
                    values: &values,
                },
            )
        };
        if let Outcome::Unreachable(why) = &done.outcome {
            return Err(ToolError::Fatal(why.clone()));
        }
        let mut report = vec![];
        if let Some(first) = first.filter(|first| !first.report.is_empty()) {
            report.push(format!("{tab}: {}", first.report));
        }
        report.push(format!("{tab}: {}", helper(&done, &values)));
        // What the tab says after, as an action leaves it (a page's changes, a terminal's new
        // output).
        let after = kind.interact(id, &json!({}))?;
        if !after.report.is_empty() {
            report.push(after.report);
        }
        Ok(ToolOutput {
            report: report.join("\n\n"),
            ..after
        })
    }
}

/// What the helper did, step by step, and how it ended; the values it never typed.
fn helper(done: &Report, values: &[String]) -> String {
    let outcome = match &done.outcome {
        Outcome::Done { probability } => format!("done ({:.0}% sure)", probability * 100.0),
        Outcome::Blocked => "blocked: nothing in the tab could make progress".into(),
        Outcome::Exhausted => "out of actions".into(),
        Outcome::Stopped(why) | Outcome::Unreachable(why) => format!("stopped: {why}"),
    };
    let mut out = format!("### Helper: {outcome}");
    for (i, step) in done.steps.iter().enumerate() {
        out.push_str(&format!("\n{}. {step}", i + 1));
    }
    let typed = |value: &String| (done.steps.iter()).any(|s| s.contains(&format!("\"{value}\"")));
    let unused: Vec<String> = (values.iter())
        .filter(|value| !typed(value))
        .map(|value| format!("\"{value}\""))
        .collect();
    if !unused.is_empty() {
        out.push_str(&format!("\nValues never typed: {}.", unused.join(", ")));
    }
    out
}
