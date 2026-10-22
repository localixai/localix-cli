//! Who answers the `ask` tool: the person at the terminal, one question at a time, on the same
//! terminal the approvals use (`gate::Tty`). An answer is a number, a number and a note, or the
//! person's own words; Enter skips. In autonomous mode nobody is asked, and the model is told at
//! once. Everything the model wrote is shown with its control characters escaped.

use std::cell::Cell;
use std::time::{Duration, Instant};

use contracts::person::{Answer, Person, Question, Reply};

use crate::gate::Prompter;
use crate::mode::{AUTONOMOUS, Mode};
use crate::printable::printable;

const NO_ANSWER: &str = "no answer came from the terminal";

pub struct TerminalPerson<'a, P: Prompter> {
    pub prompter: P,
    pub mode: &'a Cell<Mode>,
    /// Whether the person stopped the task.
    pub stopped: &'a dyn Fn() -> bool,
    /// Told how long the person took, so it does not count toward the time limit.
    pub waited: &'a dyn Fn(Duration),
}

impl<P: Prompter> Person for TerminalPerson<'_, P> {
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
            let line = self.prompter.ask(&shown(question, at, questions.len()));
            let Some(line) = line else {
                (self.waited)(asked.elapsed());
                return match (self.stopped)() {
                    true => Reply::Stopped,
                    false => Reply::Nobody(NO_ANSWER.into()),
                };
            };
            answers.push(answer(&line, question.choices.len()));
        }
        (self.waited)(asked.elapsed());
        match (self.stopped)() {
            true => Reply::Stopped,
            false => Reply::Answers(answers),
        }
    }
}

/// The question as the terminal shows it: the `at`th of `of`, its options numbered.
pub fn shown(question: &Question, at: usize, of: usize) -> String {
    let count = match of {
        1 => String::new(),
        of => format!(" ({} of {of})", at + 1),
    };
    let mut text = format!(
        "\n  ? {}{count}  {}\n",
        printable(&question.id),
        printable(&question.text)
    );
    for (number, choice) in question.choices.iter().enumerate() {
        let description = match choice.description.as_str() {
            "" => String::new(),
            description => format!("  {}", printable(description)),
        };
        text.push_str(&format!(
            "    {}. {}{description}\n",
            number + 1,
            printable(&choice.label)
        ));
    }
    text.push_str(match question.choices.is_empty() {
        true => "    Type your answer; Enter skips.\n",
        false => "    Type a number, a number and a note, or your own answer; Enter skips.\n",
    });
    text.push_str("  > ");
    text
}

/// The person's line as an answer to a question with `options` options: a number alone picks one,
/// a number and more text picks it with a note, anything else (a number out of range too) is their
/// own words, and an empty line skips.
pub fn answer(line: &str, options: usize) -> Answer {
    let line = line.trim();
    if line.is_empty() {
        return Answer::default();
    }
    let (word, rest) = line
        .split_once(|c: char| c.is_whitespace() || c == ',' || c == ':')
        .unwrap_or((line, ""));
    let number = word.trim_end_matches(['.', ')']).parse::<usize>().ok();
    match number.filter(|n| (1..=options).contains(n)) {
        Some(n) => {
            let note = rest.trim_matches(|c: char| c.is_whitespace() || c == ',' || c == ':');
            Answer {
                choice: Some(n - 1),
                note: (!note.is_empty()).then(|| note.to_string()),
            }
        }
        None => Answer {
            choice: None,
            note: Some(line.to_string()),
        },
    }
}

#[cfg(test)]
#[path = "person_tests.rs"]
mod tests;
