//! `ask`: one to three questions to the person, answered by whoever composes the tools
//! (`contracts::person::Person`). It changes nothing outside the conversation, so it asks no gate.
//! Its arguments are read leniently: a model that writes one question without the `questions`
//! list, leaves out an id or gives options as plain strings still gets its question asked, instead
//! of losing a turn to a schema error.

use contracts::person::{Answer, Choice, Person, Question, Reply};
use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use serde_json::{Map, Value, json};

const NAME: &str = "ask";
const MAX_QUESTIONS: usize = 3;
const MAX_OPTIONS: usize = 4;

const DESCRIPTION: &str = "Ask the person you work for one to three short questions, and wait \
for the answers. Use it only when the answer changes what you do and nothing you can read or run \
tells you: a preference, a tradeoff between real options, a requirement that is unclear, a value \
only they know. Look first, then ask; do not ask what you can find out, and do not ask the same \
thing twice. Offer 2 to 4 options when the choices are clear, the recommended one first with \
\" (Recommended)\" after its label; set `multiple` when several can be picked together (which \
features, which pages). The person can always add a note or answer in their own words instead. \
Never use it to ask for permission to run a command or change a file: those are approved as they \
happen. If the person skips a question or nobody can answer, go on with your best assumption and \
state it in your answer.";

pub const SKIPPED: &str = "The person skipped the question. Go on with your best assumption and \
state it in your answer; do not ask the same thing again unless it truly blocks the work.";
pub const STOPPED: &str = "the person stopped the task";

pub struct AskTool<'a> {
    pub person: &'a dyn Person,
}

impl Tool for AskTool<'_> {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME.into(),
            description: DESCRIPTION.into(),
            parameters: parameters(),
        }
    }

    fn call(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        let questions = questions(arguments).map_err(ToolError::Failed)?;
        match self.person.ask(&questions) {
            Reply::Answers(answers) => Ok(report(&questions, &answers).into()),
            Reply::Nobody(why) => Err(ToolError::Failed(why)),
            Reply::Stopped => Err(ToolError::Failed(STOPPED.into())),
        }
    }
}

fn parameters() -> Value {
    let option = json!({
        "type": "object",
        "required": ["label"],
        "additionalProperties": false,
        "properties": {
            "label": {"type": "string", "description": "1 to 5 words."},
            "description": {"type": "string",
                "description": "One short sentence: what choosing it means."},
        },
    });
    let question = json!({
        "type": "object",
        "required": ["id", "question"],
        "additionalProperties": false,
        "properties": {
            "id": {"type": "string",
                "description": "A short snake_case key the answer comes back under."},
            "question": {"type": "string",
                "description": "One sentence the person can answer without reading the code."},
            "options": {
                "type": "array",
                "maxItems": MAX_OPTIONS,
                "description": "2 to 4 choices, the recommended one first: mutually exclusive \
                    unless `multiple`. Leave out for an open question; the person can always type \
                    their own answer.",
                "items": option,
            },
            "multiple": {"type": "boolean",
                "description": "true when the person may pick several options together."},
        },
    });
    json!({
        "type": "object",
        "required": ["questions"],
        "additionalProperties": false,
        "properties": {
            "questions": {
                "type": "array",
                "minItems": 1,
                "maxItems": MAX_QUESTIONS,
                "description": "One question is best; at most three, asked in order.",
                "items": question,
            },
        },
    })
}

/// The questions `arguments` ask, or why they cannot be asked.
fn questions(arguments: &Value) -> Result<Vec<Question>, String> {
    let listed = match arguments.get("questions") {
        Some(Value::Array(list)) => list.clone(),
        Some(_) => return Err("`questions` must be a list of questions".into()),
        // One question written at the top level.
        None if arguments.get("question").is_some() => vec![arguments.clone()],
        None => return Err("give `questions`: a list of one to three questions".into()),
    };
    if listed.is_empty() {
        return Err("give at least one question".into());
    }
    if listed.len() > MAX_QUESTIONS {
        return Err(format!(
            "{} questions: ask at most {MAX_QUESTIONS} at once, the ones that matter most",
            listed.len()
        ));
    }
    listed
        .iter()
        .enumerate()
        .map(|(at, item)| question(item, at + 1))
        .collect()
}

/// The `number`th question.
fn question(item: &Value, number: usize) -> Result<Question, String> {
    let empty = Map::new();
    let (fields, text) = match item {
        Value::String(text) => (&empty, text.as_str()),
        Value::Object(fields) => (
            fields,
            fields.get("question").and_then(Value::as_str).unwrap_or(""),
        ),
        _ => return Err(format!("question {number} is not an object")),
    };
    let text = text.trim();
    if text.is_empty() {
        return Err(format!(
            "question {number} is empty: give its `question` text"
        ));
    }
    let id = match fields.get("id").and_then(Value::as_str).map(str::trim) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => format!("q{number}"),
    };
    let options = match fields.get("options") {
        None | Some(Value::Null) => vec![],
        Some(Value::Array(options)) => options.clone(),
        Some(_) => return Err(format!("the options of question {number} must be a list")),
    };
    if options.len() > MAX_OPTIONS {
        return Err(format!(
            "question {number} has {} options: offer at most {MAX_OPTIONS}",
            options.len()
        ));
    }
    let choices: Vec<Choice> = options
        .iter()
        .map(|option| choice(option, number))
        .collect::<Result<_, _>>()?;
    // Read leniently: the names other agents' ask tools use for it work too.
    let multiple = ["multiple", "multiSelect", "multi_select"]
        .iter()
        .any(|key| fields.get(*key).and_then(Value::as_bool) == Some(true));
    Ok(Question {
        id,
        text: text.to_string(),
        multiple: multiple && !choices.is_empty(),
        choices,
    })
}

/// One option: `{label, description}`, or a plain string as its label.
fn choice(option: &Value, number: usize) -> Result<Choice, String> {
    let text = |key: &str| option.get(key).and_then(Value::as_str).unwrap_or("").trim();
    let (label, description) = match option {
        Value::String(label) => (label.trim(), ""),
        _ => (text("label"), text("description")),
    };
    if label.is_empty() {
        return Err(format!("an option of question {number} has an empty label"));
    }
    Ok(Choice {
        label: label.to_string(),
        description: description.to_string(),
    })
}

/// The answers as the model reads them, one line per question in order; the skip text when
/// none was answered.
fn report(questions: &[Question], answers: &[Answer]) -> String {
    let none = Answer::default();
    let mut answered = false;
    let mut lines = vec!["The person answered:".to_string()];
    for (at, question) in questions.iter().enumerate() {
        let answer = answers.get(at).unwrap_or(&none);
        let labels: Vec<&str> = answer
            .choices
            .iter()
            .filter_map(|index| question.choices.get(*index))
            .map(|choice| choice.label.as_str())
            .collect();
        let picked = (!labels.is_empty()).then(|| labels.join(", "));
        let note = answer
            .note
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty());
        answered |= picked.is_some() || note.is_some();
        let said = match (picked, note) {
            (Some(picked), None) => picked,
            (Some(picked), Some(note)) => format!("{picked}; they added: {note}"),
            (None, Some(note)) => format!("in their words: {note}"),
            (None, None) => "no answer; use your best assumption and state it".into(),
        };
        lines.push(format!("- {}: {said}", question.id));
    }
    match answered {
        true => lines.join("\n"),
        false => SKIPPED.into(),
    }
}

#[cfg(test)]
#[path = "ask_tests.rs"]
mod tests;
