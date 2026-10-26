//! A turn's calls carried out and answered in the conversation, counted in the goal's tally; a
//! `result` call checked against its schema.

use contracts::chat::ToolCall;
use contracts::tool::{ToolError, ToolOutput, ToolSpec};
use serde_json::Value;

use super::{Agent, MAX_FIXES, RETURNED, Run, Status, Tally, Went};
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
        tally.turn = Went::Nothing;
        while start < calls.len() && !ends {
            let end = calls::batch_end(calls, start, parallel);
            if end - start == 1 {
                ends = self.handle(&calls[start], specs, run, tally);
            } else {
                let batch = &calls[start..end];
                let outcomes = calls::together(batch, self.tools, specs, self.observer);
                for (call, outcome) in batch.iter().zip(outcomes) {
                    ends |= Self::record(call, outcome, run, tally);
                }
            }
            start = end;
        }
        match tally.turn {
            Went::Succeeded => tally.failures = 0,
            Went::Failed => tally.failures += 1,
            Went::Nothing => {}
        }
        run.conversation.settle();
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
        if let (schema::TOOL, Some(schema)) = (call.name.as_str(), self.schema) {
            let (outcome, ends) = take_result(call, schema, run, &mut tally.fixes);
            self.observer.result(call, &outcome);
            answer_call(run, call, outcome);
            return ends;
        }
        let outcome = calls::invoke(self.tools, specs, call);
        self.observer.result(call, &outcome);
        Self::record(call, outcome, run, tally)
    }

    /// Count `outcome` and answer `call` with it; whether the run ends with it.
    fn record(call: &ToolCall, outcome: Outcome, run: &mut Run, tally: &mut Tally) -> bool {
        match &outcome {
            Ok(_) => tally.turn = Went::Succeeded,
            Err(ToolError::Failed(_)) => tally.turn = tally.turn.max(Went::Failed),
            Err(ToolError::Fatal(why)) => run.status = Status::Error(why.clone()),
        }
        let ends = matches!(outcome, Err(ToolError::Fatal(_)));
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
    schema: &Value,
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
            run.data = Some(Ok(done.data));
            (Ok(ToolOutput::from(RETURNED.to_string())), true)
        }
        Err(problems) if *fixes < MAX_FIXES => {
            *fixes += 1;
            let why = format!(
                "The data does not match its schema: {problems}. Call `result` again with it fixed."
            );
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
