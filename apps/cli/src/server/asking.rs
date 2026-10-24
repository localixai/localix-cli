//! The gate and the `ask` tool's person, answered in the interface: an approval as a short list,
//! a question with its options and a line for the person's own words. Nothing is approved once
//! the task is stopped, and a stop closes the question open.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use contracts::gate::Effect;
use contracts::person::{Person, Question, Reply};

use protocol::Approval;

use super::Ui;
use crate::gate::Asker;
use crate::mode::{AUTONOMOUS, Mode, ModeCell};
use crate::picker::{Adjustment, Item, Lines, Picker};

/// Approvals asked in the interface; a stopped task closes the question with a no.
pub(crate) struct UiGate<'a> {
    pub ui: Ui,
    pub stopped: &'a (dyn Fn() -> bool + Sync),
    /// The person this interface is: one question at a time (`gate::one_question`).
    pub asking: &'a Mutex<()>,
}

impl Asker for UiGate<'_> {
    fn approve(&self, effect: &Effect) -> Option<Approval> {
        let _asking = crate::gate::one_question(self.asking);
        Some(self.ui.approve(effect.tool, effect.summary, self.stopped))
    }
}

pub(crate) struct UiPerson<'a> {
    pub ui: Ui,
    pub mode: &'a ModeCell,
    pub stopped: &'a (dyn Fn() -> bool + Sync),
    /// Told how long the person took, so it does not count toward the time limit.
    pub waited: &'a (dyn Fn(Duration) + Sync),
    /// The person this interface is: one question at a time (`gate::one_question`).
    pub asking: &'a Mutex<()>,
}

impl Person for UiPerson<'_> {
    /// Each question in turn, unless the task is stopped or nobody is to be asked; the time the
    /// person took is told to `waited`. A question skipped is an empty answer.
    fn ask(&self, questions: &[Question]) -> Reply {
        if (self.stopped)() {
            return Reply::Stopped;
        }
        if self.mode.get() == Mode::Autonomous {
            return Reply::Nobody(AUTONOMOUS.into());
        }
        let asked = Instant::now();
        let _asking = crate::gate::one_question(self.asking);
        let mut answers = Vec::with_capacity(questions.len());
        for (at, question) in questions.iter().enumerate() {
            if (self.stopped)() {
                break;
            }
            let answer = self
                .ui
                .question(question, at, questions.len(), self.stopped);
            answers.push(answer.unwrap_or_default());
        }
        (self.waited)(asked.elapsed());
        match (self.stopped)() {
            true => Reply::Stopped,
            false => Reply::Answers(answers),
        }
    }
}

/// `/login`'s steps in the interface: each list a choice, each line typed in place (a key as dots).
pub(crate) struct UiTerminal {
    pub ui: Ui,
}

impl Picker for UiTerminal {
    fn pick(&mut self, title: &str, items: &[Item]) -> Option<usize> {
        let options = items
            .iter()
            .map(|item| (item.label.clone(), item.detail.clone()))
            .collect();
        self.ui
            .pick(title, options, items.iter().position(|item| item.marked))
    }

    fn pick_adjustable(
        &mut self,
        title: &str,
        items: &[Item],
        adjustments: &[Adjustment],
    ) -> Option<(usize, usize)> {
        let options = items
            .iter()
            .map(|item| (item.label.clone(), item.detail.clone()))
            .collect();
        self.ui.pick_adjustable(
            title,
            options,
            items.iter().position(|item| item.marked),
            adjustments.to_vec(),
        )
    }
}

impl Lines for UiTerminal {
    fn line(&mut self, prompt: &str, hidden: bool) -> Option<String> {
        self.ui.line(prompt, hidden, None)
    }

    fn line_until(&mut self, prompt: &str, done: &dyn Fn() -> bool) -> Option<String> {
        self.ui.line(prompt, false, Some(done))
    }
}

#[cfg(test)]
#[path = "asking_tests.rs"]
mod tests;
