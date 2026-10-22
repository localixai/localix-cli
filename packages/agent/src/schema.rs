//! `--schema`: the answer as JSON that matches a JSON Schema, for pipelines. The schema becomes a
//! tool of its own, `result`, offered from the first turn: every request carries the same tools,
//! so the prompt cache is never broken, and the run ends with the data itself rather than an extra
//! extraction call. The data is checked here; a mismatch goes back to the model to fix, and what
//! is returned always matches the schema or the run fails.

use contracts::tool::ToolSpec;
use serde_json::{Value, json};

pub const TOOL: &str = "result";

/// The `result` tool: the schema as `data`, plus whether the goal was reached.
pub fn spec(schema: &Value) -> ToolSpec {
    ToolSpec {
        name: TOOL.into(),
        description: "Finish the task: return what you found as `data`, matching its schema \
            exactly (copy numbers, names and dates from what the tools showed; null only where the \
            schema allows it and no value was shown). Call it once, when done, instead of \
            answering in text."
            .into(),
        parameters: json!({"type": "object", "required": ["data", "summary", "achieved"], "properties": {
            "data": schema,
            "summary": {"type": "string", "description": "One or two sentences for a person: what was found or done."},
            "achieved": {"type": "boolean", "description": "Whether the goal was reached."}
        }}),
    }
}

/// What the model is told in the first message (part of the cached prefix).
pub const BRIEF: &str = "When you are done, call `result` with the data its schema asks for; \
    gather every field it needs as you go.";

/// A `result` call: the data, the summary, and whether the goal was reached.
#[derive(Debug, Clone, PartialEq)]
pub struct Finished {
    pub data: Value,
    pub summary: String,
    pub achieved: bool,
}

/// A `result` call read, or what to fix.
pub fn take(args: &Value, schema: &Value) -> Result<Finished, String> {
    let data = args.get("data").cloned().unwrap_or(Value::Null);
    let mut problems = vec![];
    violations(schema, &data, "data", &mut problems);
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    Ok(Finished {
        data,
        summary: args["summary"].as_str().unwrap_or("").to_string(),
        achieved: args["achieved"].as_bool().unwrap_or(true),
    })
}

/// Where `value` breaks `schema`: type, required properties, enum, items, nested objects.
pub fn violations(schema: &Value, value: &Value, path: &str, out: &mut Vec<String>) {
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
