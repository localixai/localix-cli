//! What reaches a run between its turns. The main agent's: what the person sent while it worked
//! (first: the person comes before what they started), then the answers of the agents it started, then what its jobs
//! gave back. An agent's own: what the main agent sent it while it worked.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use contracts::inbox::Inbox;

use super::Registry;
use super::watch::Watch;
use crate::output::Output;
use crate::tui::event::Event;

/// How often a wait looks whether the person wrote or the task was stopped.
const POLL: Duration = Duration::from_millis(200);

/// The main agent's inbox.
pub struct MainInbox<'a> {
    pub registry: &'a Registry,
    pub jobs: &'a crate::jobs::Jobs,
    pub jobs_kit: &'a crate::jobs::Kit<'a>,
    pub output: &'a Output,
    pub stopped: &'a (dyn Fn() -> bool + Sync),
}

impl MainInbox<'_> {
    /// What the person sent, taken from the interface (which shows it as theirs now).
    fn steered(&self) -> Vec<String> {
        self.output
            .ui()
            .map(|ui| ui.steered(false))
            .unwrap_or_default()
    }
}

impl Inbox for MainInbox<'_> {
    fn take(&self) -> Vec<String> {
        let mut came = self.steered();
        came.extend(self.registry.take_mail());
        came.extend(self.jobs.take_mail());
        came
    }

    fn expecting(&self) -> bool {
        self.registry.working() > 0 || self.jobs.expecting(self.jobs_kit)
    }

    fn wait(&self) -> Vec<String> {
        let working = self.registry.working();
        let said = match working {
            0 => "✻ Waiting for the monitors to report\n".to_string(),
            n => format!(
                "✻ Waiting for {} to finish\n",
                crate::tui::cells::count(n as u64, "background agent")
            ),
        };
        let last = std::mem::replace(&mut *crate::output::lock(&self.registry.said), said.clone());
        if last != said {
            self.output.note(&said);
        }
        loop {
            let came = self.take();
            if !came.is_empty() || !self.expecting() || (self.stopped)() {
                return came;
            }
            self.registry.wait_change(POLL);
        }
    }
}

/// What the main agent sends an agent while it works.
#[derive(Default)]
pub struct Letters(Mutex<VecDeque<String>>);

impl Letters {
    pub fn post(&self, message: &str) {
        crate::output::lock(&self.0).push_back(message.to_string());
    }

    pub(super) fn take(&self) -> Vec<String> {
        crate::output::lock(&self.0).drain(..).collect()
    }
}

/// An agent's inbox: its letters, shown in its view as they are read.
pub struct AgentInbox<'a> {
    pub letters: &'a Letters,
    pub watch: &'a Watch<'a>,
}

impl Inbox for AgentInbox<'_> {
    fn take(&self) -> Vec<String> {
        let letters = self.letters.take();
        for letter in &letters {
            self.watch.show(Event::Note(letter.clone()));
        }
        letters
            .into_iter()
            .map(|letter| format!("The agent that started you says: {letter}"))
            .collect()
    }

    fn expecting(&self) -> bool {
        false
    }

    fn wait(&self) -> Vec<String> {
        vec![]
    }
}

#[cfg(test)]
#[path = "inbox_tests.rs"]
mod tests;
