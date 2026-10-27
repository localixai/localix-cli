//! [`Output`] as the agent's [`Observer`]: each call, its result, the model's turns, what it said.

use agent::observer::Observer;
use contracts::chat::{ToolCall, Usage};
use contracts::tool::{ToolError, ToolOutput};
use ratatui::text::{Line, Span};

use super::{Output, human, json, logged};
use crate::server::{label::label, wire};
use crate::sync::lock;
use crate::tui::tool_cell::{self, State};
use crate::tui::{cells, style, wrap};
use protocol::Event;

impl Observer for Output {
    fn call(&self, call: &ToolCall) {
        logged::call(&lock(&self.session), call);
        let step = contracts::history::Step {
            what: call.name.clone(),
            detail: wrap::cut(&tool_cell::doing(&label(&call.name, &call.arguments)), 120),
            at_ms: self.millis(),
            ..Default::default()
        };
        lock(&self.done).push((call.id.clone(), step));
        if let Some(ui) = &self.ui {
            return ui.send(Event::Call {
                call: call.id.clone(),
                name: call.name.clone(),
                label: label(&call.name, &call.arguments),
            });
        }
        if self.json {
            self.event(&json::call(call, self.millis()));
        }
        if self.trace {
            // Printed whole once the call is done: a terminal's rows cannot be redrawn here.
            lock(&self.steps).clear();
        }
    }

    fn result(&self, call: &ToolCall, outcome: &Result<ToolOutput, ToolError>) {
        logged::result(&lock(&self.session), call, outcome);
        let now = self.millis();
        let mut done = lock(&self.done);
        if let Some((_, step)) = done.iter_mut().rev().find(|(id, _)| *id == call.id) {
            step.ms = now.saturating_sub(step.at_ms);
            step.failed = outcome.is_err();
        }
        drop(done);
        if let Some(ui) = &self.ui {
            return ui.send(Event::Result {
                call: call.id.clone(),
                name: call.name.clone(),
                label: label(&call.name, &call.arguments),
                outcome: wire::outcome(outcome),
            });
        }
        if self.json {
            self.event(&json::result(call, outcome, self.millis()));
        }
        if self.trace {
            let outcome = wire::outcome(outcome);
            let done = State::Done(&outcome);
            let label = label(&call.name, &call.arguments);
            self.block(&tool_cell::head(&call.name, &label, &done, self.width));
            let steps: Vec<String> = lock(&self.steps).drain(..).collect();
            for step in steps {
                let step = wrap::cut(&format!("  · {step}"), self.width);
                self.lines(&[Line::from(Span::styled(step, style::dim()))]);
            }
            self.lines(&tool_cell::body(&call.name, &outcome, self.width));
        }
    }

    // `--json` has no event for it: the events are a contract, changed only with the schema.
    fn said(&self, text: &str) {
        logged::said(&lock(&self.session), text);
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
        logged::turn(&lock(&self.session), usage);
        let spent = contracts::history::Spent {
            input_tokens: usage.input_tokens,
            cached_tokens: usage.cached_tokens,
            output_tokens: usage.output_tokens,
            cost: usage.cost,
        };
        let step = contracts::history::Step {
            what: "model".into(),
            detail: format!("{} in · {} out", usage.input_tokens, usage.output_tokens),
            at_ms: self.millis().saturating_sub(usage.millis),
            ms: usage.millis,
            failed: false,
            spent,
        };
        lock(&self.done).push((String::new(), step));
        self.spend(spent);
        // A lane's tokens: its agents' turns come here too.
        if let Some(ui) = &self.ui {
            ui.send(Event::Spent(usage.input_tokens + usage.output_tokens));
        }
        if self.json {
            self.event(&json::turn(usage, self.millis()));
        }
        if self.trace && self.verbose {
            self.lines(&cells::note(&human::turn(usage), self.width));
        }
    }
}
