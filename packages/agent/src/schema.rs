//! The `result` tool: how every run ends. The model calls it once, when the goal is reached or
//! something outside its reach blocks it; text alone never ends a run, so a progress note sent
//! as a reply does not stop the work. With `--schema` the call also carries `data` matching the
//! schema, checked here: a mismatch goes back to the model to fix, and what is returned always
//! matches or the run says it has no data. The tool is offered from the first turn, so every
//! request carries the same tools and the prompt cache holds.

use contracts::tool::{Input, ToolSpec};
use serde_json::{Value, json};

pub(crate) const TOOL: &str = "result";

/// The `result` tool, with `data` for `schema` when there is one.
pub(crate) fn spec(schema: Option<&Value>) -> ToolSpec {
    let mut properties = json!({
        "achieved": {"type": "boolean", "description": "Whether the goal was reached."},
        "summary": {"type": "string", "description": "For a person who did not follow the work, self-contained: the outcome first, then what changed, how it was verified, what remains."},
        "blocked_by": {"type": "string", "description": "When not achieved: what outside your reach blocks the goal (missing access, a contradiction in the goal, a refused action). Work left to do is not a block."}
    });
    let mut required = vec!["achieved", "summary"];
    let mut description = String::from(
        "End the task. Call it once: when the goal is reached, or when something outside your          reach blocks it. Text alone does not end the task.",
    );
    if let Some(schema) = schema {
        properties["data"] = schema.clone();
        required.push("data");
        description.push_str(
            " Return what you found as `data`, matching its schema exactly: copy numbers,              names and dates from what the tools showed, null only where the schema allows it              and no value was shown; gather every field it needs as you go.",
        );
    }
    ToolSpec {
        name: TOOL.into(),
        description,
        input: Input::Json(
            json!({"type": "object", "required": required, "properties": properties}),
        ),
    }
}

/// A `result` call: whether the goal was reached, the summary, and the data when a schema asks.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Finished {
    pub data: Option<Value>,
    pub summary: String,
    pub achieved: bool,
}

/// A `result` call read, or what to fix.
pub(crate) fn take(args: &Value, schema: Option<&Value>) -> Result<Finished, String> {
    let achieved = args["achieved"].as_bool().unwrap_or(true);
    // A `summary` the schema owns, written beside `achieved`, is the data's, not the call's.
    let taken = schema.is_some_and(|schema| schema.pointer("/properties/summary").is_some())
        && args.get("data").is_none();
    let mut summary = match taken {
        true => String::new(),
        false => args["summary"].as_str().unwrap_or("").trim().to_string(),
    };
    if !achieved {
        // A block is named, or the call goes back: what is left to do is no block.
        let why = args["blocked_by"].as_str().map(str::trim).unwrap_or("");
        if why.is_empty() {
            return Err("not achieved needs `blocked_by`: what outside your reach blocks the goal; if nothing does, go on with the work instead".into());
        }
        summary = format!("{summary}\n\nBlocked by: {why}");
    }
    let Some(schema) = schema else {
        return Ok(Finished {
            data: None,
            summary,
            achieved,
        });
    };
    let data = match args.get("data") {
        Some(data) => data.clone(),
        None => beside(args, schema)?,
    };
    check(schema, &data)?;
    Ok(Finished {
        data: Some(data),
        summary,
        achieved,
    })
}

/// A call whose fields sit beside `achieved` instead of inside `data`. Models do it when the
/// schema has a field the call has too (a description's `summary`): the fields are the data when
/// they match the schema, and otherwise the model is told where they go.
fn beside(args: &Value, schema: &Value) -> Result<Value, String> {
    let mut fields = args.as_object().cloned().unwrap_or_default();
    for own in ["achieved", "blocked_by"] {
        fields.remove(own);
    }
    if schema.pointer("/properties/summary").is_none() {
        fields.remove("summary");
    }
    let data = Value::Object(fields);
    check(schema, &data)
        .map_err(|why| format!("no `data`: put the schema's fields inside `data` ({why})"))?;
    Ok(data)
}

/// Whether `data` matches `schema`, or every way it does not.
fn check(schema: &Value, data: &Value) -> Result<(), String> {
    let mut problems = vec![];
    violations(schema, data, "data", &mut problems);
    match problems.is_empty() {
        true => Ok(()),
        false => Err(problems.join("; ")),
    }
}

/// Where `value` breaks `schema`: type, required properties, enum, items, nested objects.
pub(crate) fn violations(schema: &Value, value: &Value, path: &str, out: &mut Vec<String>) {
    let at = if path.is_empty() { "the result" } else { path };
    if let Some(options) = schema.get("enum").and_then(Value::as_array)
        && !options.contains(value)
    {
        out.push(format!(
            "{at} must be one of {}",
            Value::Array(options.clone())
        ));
    }
    let types: Vec<&str> = match schema.get("type") {
        Some(Value::String(one)) => vec![one.as_str()],
        Some(Value::Array(many)) => many.iter().filter_map(Value::as_str).collect(),
        _ => vec![],
    };
    if !types.is_empty() && !types.iter().any(|kind| matches_type(kind, value)) {
        out.push(format!("{at} must be {}", types.join(" or ")));
        return;
    }
    if let (Some(object), Some(properties)) = (
        value.as_object(),
        schema.get("properties").and_then(Value::as_object),
    ) {
        for name in schema
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !object.contains_key(name) {
                out.push(format!("{at} is missing \"{name}\""));
            }
        }
        for (name, sub) in properties {
            if let Some(child) = object.get(name) {
                violations(sub, child, &format!("{path}.{name}"), out);
            }
        }
    }
    if let (Some(items), Some(schema_items)) = (value.as_array(), schema.get("items")) {
        for (i, item) in items.iter().enumerate() {
            violations(schema_items, item, &format!("{path}[{i}]"), out);
        }
    }
}

fn matches_type(kind: &str, value: &Value) -> bool {
    match kind {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_f64().is_some_and(|n| n.fract() == 0.0),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => true,
    }
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
