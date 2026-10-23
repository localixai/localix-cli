//! Carrying out a turn's calls. Calls whose tools allow it run at the same time, each on a thread
//! of its own; any other call runs alone, in the order the model wrote it, so what it changes is
//! seen by the calls after it. Results go back in the model's order, whatever order they end in.

use std::sync::mpsc;

use contracts::chat::ToolCall;
use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};

use crate::observer::Observer;

pub(crate) type Outcome = Result<ToolOutput, ToolError>;

/// The calls from `start` on that run together: the run of calls that may run in parallel, or
/// the one call at `start`; the index after them.
pub(crate) fn batch_end(
    calls: &[ToolCall],
    start: usize,
    parallel: impl Fn(&ToolCall) -> bool,
) -> usize {
    if !parallel(&calls[start]) {
        return start + 1;
    }
    let mut end = start + 1;
    while end < calls.len() && parallel(&calls[end]) {
        end += 1;
    }
    end
}

/// Whether `call` may run beside others: its tool says so and its arguments can be read.
pub(crate) fn parallel(tools: &[&dyn Tool], specs: &[ToolSpec], call: &ToolCall) -> bool {
    call.arguments.is_object() && find(tools, specs, call).is_some_and(|tool| tool.parallel())
}

/// `calls`, all at once; their outcomes in the same order. Each is shown to `observer` as it
/// starts and as it ends.
pub(crate) fn together(
    calls: &[ToolCall],
    tools: &[&dyn Tool],
    specs: &[ToolSpec],
    observer: &dyn Observer,
) -> Vec<Outcome> {
    for call in calls {
        observer.call(call);
    }
    let mut outcomes: Vec<Option<Outcome>> = calls.iter().map(|_| None).collect();
    // One send per call: room for all, so no call waits on the reader.
    let (done, ended) = mpsc::sync_channel(calls.len());
    std::thread::scope(|scope| {
        for (at, call) in calls.iter().enumerate() {
            let done = done.clone();
            scope.spawn(move || {
                let _ = done.send((at, invoke(tools, specs, call)));
            });
        }
        drop(done);
        for (at, outcome) in ended {
            observer.result(&calls[at], &outcome);
            outcomes[at] = Some(outcome);
        }
    });
    outcomes
        .into_iter()
        .map(|outcome| {
            outcome.unwrap_or_else(|| Err(ToolError::Failed("the call did not finish".into())))
        })
        .collect()
}

/// Run `call` on the tool it names; `specs` are the tools' specs, in the same order.
pub(crate) fn invoke(tools: &[&dyn Tool], specs: &[ToolSpec], call: &ToolCall) -> Outcome {
    let tool = find(tools, specs, call)
        .ok_or_else(|| ToolError::Failed(format!("there is no tool {}", call.name)))?;
    if !call.arguments.is_object() {
        return Err(ToolError::Failed(
            "the arguments are not a JSON object (a reply cut short leaves them incomplete): \
             call the tool again, and write a large file in parts"
                .into(),
        ));
    }
    tool.call(&call.arguments)
}

fn find<'a>(tools: &[&'a dyn Tool], specs: &[ToolSpec], call: &ToolCall) -> Option<&'a dyn Tool> {
    specs
        .iter()
        .position(|spec| spec.name == call.name)
        .and_then(|at| tools.get(at).copied())
}

#[cfg(test)]
#[path = "calls_tests.rs"]
mod tests;
