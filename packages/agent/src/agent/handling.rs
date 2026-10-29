//! A turn's calls carried out and answered in the conversation.

use contracts::chat::ToolCall;
use contracts::tool::{ToolError, ToolSpec};

use super::{Agent, Run, Status};
use crate::calls::{self, Outcome};

impl Agent<'_> {
    /// Carry out a turn's calls, those that may run together at once; whether the run ends.
    pub(super) fn handle_all(&self, calls: &[ToolCall], specs: &[ToolSpec], run: &mut Run) -> bool {
        let parallel = |call: &ToolCall| calls::parallel(self.tools, specs, call);
        let mut ends = false;
        let mut start = 0;
        while start < calls.len() && !ends {
            let end = calls::batch_end(calls, start, parallel);
            if end - start == 1 {
                ends = self.handle(&calls[start], specs, run);
            } else {
                let batch = &calls[start..end];
                let outcomes = calls::together(batch, self.tools, specs, self.observer);
                for (call, outcome) in batch.iter().zip(outcomes) {
                    ends |= Self::record(call, outcome, run);
                }
            }
            start = end;
        }
        ends
    }

    /// Carry out `call`, answer it in the conversation, and say whether the run ends with it.
    fn handle(&self, call: &ToolCall, specs: &[ToolSpec], run: &mut Run) -> bool {
        self.observer.call(call);
        let outcome = calls::invoke(self.tools, specs, call);
        self.observer.result(call, &outcome);
        Self::record(call, outcome, run)
    }

    /// Answer `call` with `outcome`; whether the run ends with it (a fatal failure).
    fn record(call: &ToolCall, outcome: Outcome, run: &mut Run) -> bool {
        let ends = match &outcome {
            Ok(_) | Err(ToolError::Failed(_)) => false,
            Err(ToolError::Fatal(why)) => {
                run.status = Status::Error(why.clone());
                true
            }
        };
        answer_call(run, call, outcome);
        ends
    }
}

/// Answer `call` in the conversation.
fn answer_call(run: &mut Run, call: &ToolCall, outcome: Outcome) {
    match outcome {
        Ok(output) => run.conversation.result(&call.id, output),
        Err(error) => run.conversation.failure(&call.id, &error.to_string()),
    }
}
