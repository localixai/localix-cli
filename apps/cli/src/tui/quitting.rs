//! Quitting while work runs: Ctrl-C twice, Ctrl-D and `/exit` ask first when a session works or
//! a shell or a monitor runs, since quitting stops them all. "Quit anyway" stops every session
//! and ends llx; "Stay" (or Esc) closes the question.
//!
//! ```text
//! • Quit llx?
//!   2 sessions still working · 3 shells and 1 monitor running — they will be stopped.
//!
//! ❯ 1. Quit anyway
//!   2. Stay
//! ```

use std::time::Instant;

use ratatui::text::{Line, Span};

use super::app::{Action, App, Open};
use super::cells::count;
use super::queue::Asked;
use super::select::{Choice, Picked, Select};
use super::style;

impl App {
    /// The person asks to quit: at once when nothing would be stopped, else asked first.
    pub(super) fn quit(&mut self) -> Option<Action> {
        let Some(said) = self.running_work() else {
            return Some(Action::Quit);
        };
        if self.questions.iter().any(|q| matches!(q.open, Open::Quit)) {
            return None;
        }
        let title = vec![
            Line::from(vec![
                Span::styled("• ", style::waiting()),
                Span::styled("Quit llx?", style::bold()),
            ]),
            Line::from(Span::styled(format!("  {said}"), style::dim())),
        ];
        let choices = vec![Choice::new("Quit anyway"), Choice::new("Stay")];
        let select = Select::new(title, choices, "esc to stay");
        self.questions.push_back(Asked::new(select, Open::Quit));
        if self.questions.len() == 1 {
            self.settling = None;
        }
        None
    }

    /// Whether a lane other than the one shown works.
    pub(super) fn others_work(&self) -> bool {
        let home = self.views.home();
        let mut lanes = self.views.sessions.iter().enumerate();
        lanes.any(|(at, view)| at != home && view.lane.is_some() && view.busy.is_some())
    }

    /// What quitting would stop, said as the question says it; `None` when nothing.
    fn running_work(&self) -> Option<String> {
        let lanes = self
            .views
            .sessions
            .iter()
            .filter(|view| view.lane.is_some());
        let working = lanes.filter(|view| view.busy.is_some()).count();
        let shells = (self.shells)();
        let monitors = self
            .views
            .jobs
            .iter()
            .filter(|job| job.until_at.is_some())
            .count();
        if working + shells + monitors == 0 {
            return None;
        }
        let mut parts = vec![];
        if working > 0 {
            parts.push(format!(
                "{} still working",
                count(working as u64, "session")
            ));
        }
        let running: Vec<String> = [(shells, "shell"), (monitors, "monitor")]
            .into_iter()
            .filter(|(n, _)| *n > 0)
            .map(|(n, noun)| count(n as u64, noun))
            .collect();
        if !running.is_empty() {
            parts.push(format!("{} running", running.join(" and ")));
        }
        let them = match working + shells + monitors {
            1 => "it",
            _ => "they",
        };
        Some(format!("{} — {them} will be stopped.", parts.join(" · ")))
    }

    /// The quit question took `picked`: "Quit anyway" quits, anything else stays.
    pub(super) fn quit_answered(&mut self, picked: &Picked, now: Instant) -> Option<Action> {
        self.answer_open(Picked::Cancelled, now);
        (*picked == Picked::Option(0)).then_some(Action::QuitNow)
    }
}

#[cfg(test)]
#[path = "quitting_tests.rs"]
mod tests;
