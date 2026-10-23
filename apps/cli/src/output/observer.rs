//! [`Output`] as the agent's [`Observer`]: each call, its result, the model's turns, what it said.

use agent::observer::Observer;
use contracts::chat::{ToolCall, Usage};
use contracts::tool::{ToolError, ToolOutput};
use ratatui::text::{Line, Span};

use super::{Output, human, json, lock};
use crate::tui::event::Event;
use crate::tui::tool_cell::{self, State};
use crate::tui::{cells, style, wrap};

impl Observer for Output {
    fn call(&self, call: &ToolCall) {
        let step = contracts::history::Step {
            what: call.name.clone(),
            detail: wrap::cut(&tool_cell::doing(&call.name, &call.arguments), 120),
            at_ms: self.millis(),
            ..Default::default()
        };
        lock(&self.done).push((call.id.clone(), step));
        if let Some(ui) = &self.ui {
            return ui.send(Event::Call {
                name: call.name.clone(),
                args: call.arguments.clone(),
            });
        }
        if self.json {
            self.event(json::call(call, self.millis()));
        }
        if self.trace {
            // Printed whole once the call is done: a terminal's rows cannot be redrawn here.
            lock(&self.steps).clear();
        }
    }

    fn result(&self, call: &ToolCall, outcome: &Result<ToolOutput, ToolError>) {
        let now = self.millis();
        let mut done = lock(&self.done);
        if let Some((_, step)) = done.iter_mut().rev().find(|(id, _)| *id == call.id) {
            step.ms = now.saturating_sub(step.at_ms);
            step.failed = outcome.is_err();
        }
        drop(done);
        if let Some(ui) = &self.ui {
            return ui.send(Event::Result {
                name: call.name.clone(),
                args: call.arguments.clone(),
                outcome: outcome.clone(),
            });
        }
        if self.json {
            self.event(json::result(call, outcome, self.millis()));
        }
        if self.trace {
            let done = State::Done(outcome);
            self.block(&tool_cell::head(
                &call.name,
                &call.arguments,
                &done,
                self.width,
            ));
            let steps: Vec<String> = lock(&self.steps).drain(..).collect();
            for step in steps {
                let step = wrap::cut(&format!("  · {step}"), self.width);
                self.lines(&[Line::from(Span::styled(step, style::dim()))]);
            }
            self.lines(&tool_cell::body(&call.name, outcome, self.width));
        }
    }

    // `--json` has no event for it: the events are a contract, changed only with the schema.
    fn said(&self, text: &str) {
        if let Some(ui) = &self.ui {
            return ui.send(Event::Answer(text.to_string()));
        }
        if self.trace {
            self.block(&cells::agent(text, self.width));
        }
    }

    fn fitted(&self, report: &agent::history::Report) {
        let said: Vec<String> = report
            .iter()
            .map(|(stage, count)| format!("{count} {stage}"))
            .collect();
        self.note(&format!(
            "✻ The context neared the model's limit; elided to save it: {}\n",
            said.join(", ")
        ));
    }

    fn turn(&self, usage: &Usage) {
        let step = contracts::history::Step {
            what: "model".into(),
            detail: format!("{} in · {} out", usage.input_tokens, usage.output_tokens),
            at_ms: self.millis().saturating_sub(usage.millis),
            ms: usage.millis,
            failed: false,
        };
        lock(&self.done).push((String::new(), step));
        // A lane's tokens: its agents' turns come here too.
        if let Some(ui) = &self.ui {
            ui.send(Event::Spent(usage.input_tokens + usage.output_tokens));
        }
        if self.json {
            self.event(json::turn(usage, self.millis()));
        }
        if self.trace && self.verbose {
            self.lines(&cells::note(&human::turn(usage), self.width));
        }
    }
}
