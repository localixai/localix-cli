//! The questions the session puts to the person, as choices: an approval, one of the model's
//! questions, a choice of llx's own; and the answer each pick sends back.

use protocol::{Answer, Question, Reply};
use ratatui::text::{Line, Span};

use super::app::Open;
use super::event::Approval;
use super::select::{Choice, Picked, Select};
use super::style;
use crate::tui::printable::printable;

/// `label` names the session asking when it is not the one shown.
pub(crate) fn approval(tool: &str, summary: &str, label: Option<&str>) -> Select {
    let title = match tool {
        "bash" => "Run this command?".to_string(),
        "apply_patch" => "Change this file?".to_string(),
        tool => format!("Allow {}?", printable(tool)),
    };
    let tool = printable(tool);
    let mut title = vec![heading(&title, "")];
    title.extend(label.map(asker));
    title.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(printable(summary), style::code()),
    ]));
    Select::new(
        title,
        vec![
            Choice::new("Yes"),
            Choice::new(&format!("Yes, and don't ask again for {tool}")),
            Choice::new("No, and tell llx what to do differently").types(),
        ],
        "esc no · ctrl-c stop",
    )
}

/// The `at`th of `of` questions: its options, then the person's own words.
pub(crate) fn question(question: &Question, at: usize, of: usize, label: Option<&str>) -> Select {
    let count = match of {
        1 => String::new(),
        of => format!("  {} of {of}", at + 1),
    };
    let mut title = vec![heading(&printable(&question.text), &count)];
    title.extend(label.map(asker));
    let mut choices: Vec<Choice> = question
        .choices
        .iter()
        .map(|choice| Choice::new(&choice.label).detail(&choice.description))
        .collect();
    let own = choices.len();
    choices.push(Choice::new("Something else: type it").types());
    let select = Select::new(title, choices, "esc skip").multiple(question.multiple && own > 1);
    if own == 0 { select.typing(0) } else { select }
}

pub(crate) fn choice(title: &str, options: &[(String, String)], marked: Option<usize>) -> Select {
    let title = vec![heading(&printable(title), "")];
    let choices = options
        .iter()
        .map(|(label, detail)| Choice::new(label).detail(detail))
        .collect();
    Select::new(title, choices, "esc cancel").at(marked.unwrap_or(0))
}

/// A line to type, its characters hidden for a secret. A step's prompt (`Login › OpenAI › API
/// key: paste it…`) is its path as the heading, and what to do under it.
pub(crate) fn line(prompt: &str, hidden: bool) -> Select {
    let prompt = printable(prompt.trim().trim_end_matches(['>', ':']).trim());
    let mut title = vec![];
    match prompt
        .split_once(": ")
        .filter(|(path, _)| path.contains(" › "))
    {
        Some((path, what)) => {
            title.push(heading(path, ""));
            title.push(Line::from(Span::styled(format!("  {what}"), style::dim())));
        }
        None => title.push(heading(&prompt, "")),
    }
    let select = Select::new(title, vec![Choice::new("Type it").types()], "esc cancel");
    select.hidden(hidden).typing(0)
}

/// `session <name> (in the background)`: who asks, when it is not the session shown.
fn asker(name: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled("  session ", style::dim()),
        Span::styled(printable(name), style::bold()),
        Span::styled(" (in the background)", style::dim()),
    ])
}

/// A question's first line: `• title`, and `after` dimmed.
pub(super) fn heading(title: &str, after: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled("• ", style::waiting()),
        Span::styled(title.to_string(), style::bold()),
        Span::styled(after.to_string(), style::dim()),
    ])
}

/// What `picked` means to whoever asked, as the reply under its question's id; and, for a
/// refused approval, what the person said (empty for nothing), for the call's block.
pub(crate) fn answer(open: &Open, picked: Picked) -> (Option<(u64, Reply)>, Option<String>) {
    let (id, reply) = match *open {
        Open::Quit => return (None, None),
        Open::Approval(id) => {
            let approval = match picked {
                Picked::Option(0) => Approval::Yes,
                Picked::Option(1) => Approval::Always,
                Picked::Typed(_, why) if !why.is_empty() => Approval::No(Some(why)),
                Picked::Option(_)
                | Picked::Adjusted(..)
                | Picked::Many(_)
                | Picked::Typed(..)
                | Picked::Cancelled => Approval::No(None),
            };
            let said = match &approval {
                Approval::No(why) => Some(why.clone().unwrap_or_default()),
                Approval::Yes | Approval::Always => None,
            };
            return (Some((id, Reply::Approval(approval))), said);
        }
        Open::Question(id) => (id, Reply::Question(answered(picked))),
        Open::Line(id) => (
            id,
            Reply::Line(match picked {
                Picked::Typed(_, text) => Some(text),
                Picked::Option(_) | Picked::Adjusted(..) | Picked::Many(_) | Picked::Cancelled => {
                    None
                }
            }),
        ),
        Open::Adjustable(id) => (
            id,
            Reply::Adjusted(match picked {
                Picked::Adjusted(at, value) => Some((at, value)),
                Picked::Option(_) | Picked::Many(_) | Picked::Typed(..) | Picked::Cancelled => None,
            }),
        ),
        Open::Choice(id) => (
            id,
            Reply::Choice(match picked {
                Picked::Option(at) => Some(at),
                Picked::Adjusted(..) | Picked::Many(_) | Picked::Typed(..) | Picked::Cancelled => {
                    None
                }
            }),
        ),
    };
    (Some((id, reply)), None)
}

/// A question of the model's as the person answered it: an option, their words, several
/// options, or nothing (skipped); `None` when they cancelled it.
fn answered(picked: Picked) -> Option<Answer> {
    match picked {
        Picked::Option(at) => Some(Answer {
            choices: vec![at],
            note: None,
        }),
        Picked::Typed(_, text) if !text.is_empty() => Some(Answer {
            choices: vec![],
            note: Some(text),
        }),
        Picked::Typed(..) => Some(Answer::default()),
        Picked::Many(picked) => Some(Answer {
            choices: picked,
            note: None,
        }),
        Picked::Cancelled | Picked::Adjusted(..) => None,
    }
}

#[cfg(test)]
#[path = "questions_tests.rs"]
mod tests;
