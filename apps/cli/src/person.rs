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
            answers.push(answer(&line, question.choices.len(), question.multiple));
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
    text.push_str(match (question.choices.is_empty(), question.multiple) {
        (true, _) => "    Type your answer; Enter skips.\n",
        (false, false) => "    Type a number, a number and a note, or your own answer; Enter skips.\n",
        (false, true) => {
            "    Type one or more numbers (1,3), with a note after them, or your own answer; Enter skips.\n"
        }
    });
    text.push_str("  > ");
    text
}

/// The person's line as an answer to a question with `options` options: a number alone picks one,
/// a number and more text picks it with a note, anything else (a number out of range too) is their
/// own words, and an empty line skips. With `multiple`, every number at the start picks one
/// ("1, 3 and a note").
pub fn answer(line: &str, options: usize, multiple: bool) -> Answer {
    let line = line.trim();
    if line.is_empty() {
        return Answer::default();
    }
    let separator = |c: char| c.is_whitespace() || c == ',' || c == ':';
    let mut choices: Vec<usize> = vec![];
    let mut rest = line;
    loop {
        let (word, after) = rest.split_once(separator).unwrap_or((rest, ""));
        let number = word.trim_end_matches(['.', ')']).parse::<usize>().ok();
        match number.filter(|n| (1..=options).contains(n)) {
            Some(n) if choices.is_empty() || multiple => {
                if !choices.contains(&(n - 1)) {
                    choices.push(n - 1);
                }
                rest = after.trim_start_matches(separator);
            }
            _ => break,
        }
        if rest.is_empty() {
            break;
        }
    }
    if choices.is_empty() {
        return Answer {
            choices,
            note: Some(line.to_string()),
        };
    }
    let note = rest.trim_matches(separator);
    Answer {
        choices,
        note: (!note.is_empty()).then(|| note.to_string()),
    }
}

#[cfg(test)]
#[path = "person_tests.rs"]
mod tests;
