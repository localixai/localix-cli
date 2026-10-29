//! A turn's calls carried out and answered in the conversation; a `result` call read, and its
//! data checked against the schema.

use contracts::chat::ToolCall;
use contracts::tool::{ToolError, ToolOutput, ToolSpec};
use serde_json::Value;

use super::{Agent, MAX_FIXES, RETURNED, Run, Status, Tally};
use crate::calls::{self, Outcome};
use crate::schema;

impl Agent<'_> {
    /// Carry out a turn's calls, those that may run together at once; whether the run ends.
    pub(super) fn handle_all(
        &self,
        calls: &[ToolCall],
        specs: &[ToolSpec],
        run: &mut Run,
        tally: &mut Tally,
    ) -> bool {
        let parallel =
            |call: &ToolCall| call.name != schema::TOOL && calls::parallel(self.tools, specs, call);
        let mut ends = false;
        let mut start = 0;
        while start < calls.len() && !ends {
            let end = calls::batch_end(calls, start, parallel);
            if end - start == 1 {
                ends = self.handle(&calls[start], specs, run, tally);
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
    fn handle(
        &self,
        call: &ToolCall,
        specs: &[ToolSpec],
        run: &mut Run,
        tally: &mut Tally,
    ) -> bool {
        self.observer.call(call);
        if call.name == schema::TOOL {
            let (outcome, ends) = take_result(call, self.schema, run, &mut tally.fixes);
            self.observer.result(call, &outcome);
            answer_call(run, call, outcome);
            return ends;
        }
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

/// A `result` call: data that matches ends the run; a mismatch goes back to be fixed, until the
/// fixes run out and the run ends without data. Returns its outcome and whether the run ends.
fn take_result(
    call: &ToolCall,
    schema: Option<&Value>,
    run: &mut Run,
    fixes: &mut usize,
) -> (Result<ToolOutput, ToolError>, bool) {
    match schema::take(&call.arguments, schema) {
        Ok(done) => {
            run.status = if done.achieved {
                Status::Achieved
            } else {
                Status::NotAchieved
            };
            run.answer = done.summary;
            run.data = done.data.map(Ok);
            (Ok(ToolOutput::from(RETURNED.to_string())), true)
        }
        Err(problems) if *fixes < MAX_FIXES => {
            *fixes += 1;
            let why = format!("The result is not taken: {problems}. Call `result` again.");
            (Err(ToolError::Failed(why)), false)
        }
        Err(problems) => {
            let why = format!("the result did not match the schema: {problems}");
            run.status = Status::NotAchieved;
            run.data = Some(Err(why.clone()));
            (Err(ToolError::Failed(why)), true)
        }
    }
}
