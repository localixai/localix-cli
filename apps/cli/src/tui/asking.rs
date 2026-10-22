//! The gate and the `ask` tool's person, answered in the interface: an approval as a short list,
//! a question with its options and a line for the person's own words. Nothing is approved once
//! the task is stopped, and a stop closes the question open.

use std::cell::Cell;
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use contracts::gate::{Effect, Gate, Verdict};
use contracts::person::{Person, Question, Reply};

use super::Ui;
use super::event::Approval;
use crate::gate::declined;
use crate::mode::{AUTONOMOUS, Mode};

pub struct UiGate<'a> {
    pub ui: Ui,
    pub stopped: &'a dyn Fn() -> bool,
    /// The tools the person said not to ask about again.
    always: Mutex<HashSet<String>>,
}

impl<'a> UiGate<'a> {
    pub fn new(ui: Ui, stopped: &'a dyn Fn() -> bool) -> Self {
        Self {
            ui,
            stopped,
            always: Mutex::new(HashSet::new()),
        }
    }
}

impl Gate for UiGate<'_> {
    fn check(&self, effect: Effect) -> Verdict {
        if self.always.lock().unwrap().contains(effect.tool) {
            return Verdict::Allow;
        }
        match self.ui.approve(effect.tool, effect.summary, self.stopped) {
            Approval::Yes => Verdict::Allow,
            Approval::Always => {
                self.always.lock().unwrap().insert(effect.tool.to_string());
                Verdict::Allow
            }
            Approval::No(why) => Verdict::Deny(declined(why)),
        }
    }
}

pub struct UiPerson<'a> {
    pub ui: Ui,
    pub mode: &'a Cell<Mode>,
    pub stopped: &'a dyn Fn() -> bool,
    /// Told how long the person took, so it does not count toward the time limit.
    pub waited: &'a dyn Fn(Duration),
}

impl Person for UiPerson<'_> {
    fn ask(&self, questions: &[Question]) -> Reply {
        if (self.stopped)() {
            return Reply::Stopped;
        }
        if self.mode.get() == Mode::Autonomous {
            return Reply::Nobody(AUTONOMOUS.into());
        }
        let asked = Instant::now();
        let mut answers = Vec::with_capacity(questions.len());
        for (at, question) in questions.iter().enumerate() {
            let answer = self.ui.ask(question, at, questions.len(), self.stopped);
            if (self.stopped)() {
                break;
            }
            answers.push(answer.unwrap_or_default());
        }
        (self.waited)(asked.elapsed());
        match (self.stopped)() {
            true => Reply::Stopped,
            false => Reply::Answers(answers),
        }
    }
}
