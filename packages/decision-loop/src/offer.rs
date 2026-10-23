//! What a step can offer: the controls in scope (a dialog's alone while one is open), the
//! operations they allow now, the (field, value) pairs worth typing, and the targets the view
//! refused as covered.

use serde_json::{Map, Value, json};

use contracts::view::Element;

use crate::step::{GoalStep, OPERATIONS, Operation};

pub(crate) fn clickable(element: &Element) -> bool {
    !element.disabled()
}

fn editable(element: &Element) -> bool {
    clickable(element) && element.editable
}

pub(crate) fn dropdown(element: &Element) -> bool {
    clickable(element) && !element.options.is_empty()
}

/// While a dialog is open, only its controls are offered: it blocks the rest of the view.
pub fn in_scope(elements: &[Element]) -> Vec<&Element> {
    let dialog: Vec<&Element> = elements.iter().filter(|element| element.dialog).collect();
    if dialog.is_empty() {
        elements.iter().collect()
    } else {
        dialog
    }
}

/// The operations available in this view, now.
pub(crate) fn operations_now(
    elements: &[&Element],
    typable: bool,
    history: &[GoalStep],
    busy: bool,
) -> Vec<Operation> {
    let last = history.last();
    OPERATIONS
        .into_iter()
        .filter(|operation| match operation {
            Operation::Click => elements.iter().any(|e| clickable(e)),
            Operation::TypeText => typable,
            Operation::Select => elements.iter().any(|e| dropdown(e)),
            Operation::PressEnter => last.is_some_and(|step| step.operation == Operation::TypeText),
            // Waiting helps while the view says it is working, or right after an action that has
            // shown nothing yet (a slow answer). On a settled view a WAIT only defers DONE: after
            // "click Next and wait for the next question" it waited, then answered more.
            Operation::Wait => {
                busy || last.is_some_and(|step| {
                    step.operation != Operation::Wait && step.changed == Some(false)
                })
            }
            // A scroll that moved nothing is not offered again that way; three in a row end it.
            Operation::ScrollDown | Operation::ScrollUp => {
                let repeated = last.is_some_and(|step| {
                    step.operation == *operation && step.changed == Some(false)
                });
                let recent = &history[history.len().saturating_sub(3)..];
                let scrolling =
                    recent.len() == 3 && recent.iter().all(|step| step.operation.scroll());
                !(repeated || scrolling)
            }
            _ => true,
        })
        .collect()
}

pub(crate) fn criteria<'a>(elements: impl Iterator<Item = &'a &'a Element>) -> Map<String, Value> {
    elements
        .map(|element| (element.reference.clone(), json!(element.describe())))
        .collect()
}

/// Every (field, value) pair worth typing, as one choice: a field and a value chosen apart can
/// pair a city with a date field. A field that holds one of the values is filled, and a value a
/// field holds is placed: neither is offered again, so a filled form never trades values between
/// its fields (name into e-mail, e-mail into name).
pub(crate) fn entries(elements: &[&Element], values: &[String]) -> Vec<(String, String, String)> {
    let holds = |element: &Element, value: &str| {
        element
            .value
            .as_deref()
            .is_some_and(|now| now.trim().eq_ignore_ascii_case(value.trim()))
    };
    let editable: Vec<&Element> = elements.iter().copied().filter(|e| editable(e)).collect();
    let placed = |value: &str| editable.iter().any(|e| holds(e, value));
    let mut out = vec![];
    for element in editable
        .iter()
        .filter(|e| !values.iter().any(|v| holds(e, v)))
    {
        for (i, value) in values.iter().enumerate() {
            if !placed(value) && out.len() < MAX_ENTRIES {
                let key = format!("{}=v{}", element.reference, i + 1);
                let text = format!("{} ← \"{value}\"", element.describe());
                out.push((key, element.reference.clone(), text));
            }
        }
    }
    out
}

const MAX_ENTRIES: usize = 80;

/// The targets the view refused as covered since it last changed: clicking them again only fails
/// again, so the decision goes to what covers them (a dialog the next look offers) instead, as
/// long as something else is offered.
pub(crate) fn covered(history: &[GoalStep]) -> Vec<&str> {
    history
        .iter()
        .rev()
        .take_while(|step| step.changed != Some(true))
        .filter(|step| {
            step.failed
                .as_deref()
                .is_some_and(|e| e.starts_with("covered"))
        })
        .filter_map(|step| step.reference.as_deref())
        .collect()
}
