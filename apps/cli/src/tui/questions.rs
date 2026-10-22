//! The questions the session puts to the person, as choices: an approval, one of the model's
//! questions, a choice of llx's own; and the answer each pick sends back.

use contracts::person::{Answer, Question};
use ratatui::text::{Line, Span};

use super::app::Open;
use super::event::Approval;
use super::select::{Choice, Picked, Select};
use super::style;
use crate::printable::printable;

pub fn approval(tool: &str, summary: &str) -> Select {
    let title = match tool {
        "bash" => "Run this command?".to_string(),
        "edit" => "Change this file?".to_string(),
        "browser" => "Let the browser do this?".to_string(),
        tool => format!("Allow {}?", printable(tool)),
    };
    let tool = printable(tool);
    Select::new(
        vec![
            Line::from(vec![
                Span::styled("• ", style::waiting()),
                Span::styled(title, style::bold()),
            ]),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(printable(summary), style::code()),
            ]),
        ],
        vec![
            Choice::new("Yes").key('y'),
            Choice::new(&format!("Yes, and don't ask again for {tool}")).key('a'),
            Choice::new("No, and tell llx what to do differently")
                .key('n')
                .types(),
        ],
        "esc to say no",
    )
}

/// The `at`th of `of` questions: its options, then the person's own words.
pub fn question(question: &Question, at: usize, of: usize) -> Select {
    let count = match of {
        1 => String::new(),
        of => format!("  {} of {of}", at + 1),
    };
    let title = vec![Line::from(vec![
        Span::styled("• ", style::waiting()),
        Span::styled(printable(&question.text), style::bold()),
        Span::styled(count, style::dim()),
    ])];
    let mut choices: Vec<Choice> = question
        .choices
        .iter()
        .map(|choice| Choice::new(&choice.label).detail(&choice.description))
        .collect();
    let own = choices.len();
    choices.push(Choice::new("Something else: type it").types());
    let select = Select::new(title, choices, "esc to skip");
    if own == 0 { select.typing(0) } else { select }
}

pub fn choice(title: &str, options: &[String]) -> Select {
    let title = vec![Line::from(vec![
        Span::styled("• ", style::waiting()),
        Span::styled(printable(title), style::bold()),
    ])];
    Select::new(
        title,
        options.iter().map(|option| Choice::new(option)).collect(),
        "esc to cancel",
    )
}

/// Sends what `picked` means to whoever asked.
pub fn answer(open: Open, picked: Picked) {
    match open {
        Open::Approval(reply) => {
            let approval = match picked {
                Picked::Option(0) => Approval::Yes,
                Picked::Option(1) => Approval::Always,
                Picked::Typed(_, why) if !why.is_empty() => Approval::No(Some(why)),
                _ => Approval::No(None),
            };
            let _ = reply.send(approval);
        }
        Open::Question(reply) => {
            let answer = match picked {
                Picked::Option(at) => Some(Answer {
                    choice: Some(at),
                    note: None,
                }),
                Picked::Typed(_, text) if !text.is_empty() => Some(Answer {
                    choice: None,
                    note: Some(text),
                }),
                Picked::Typed(..) => Some(Answer::default()),
                Picked::Cancelled => None,
            };
            let _ = reply.send(answer);
        }
        Open::Choice(reply) => {
            let _ = reply.send(match picked {
                Picked::Option(at) => Some(at),
                _ => None,
            });
        }
    }
}
