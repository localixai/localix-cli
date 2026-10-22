//! `--schema`: the answer as JSON that matches a JSON Schema, for pipelines. The schema becomes a
//! second tool, `result`, offered from the first turn: every request carries the same tools, so
//! the prompt cache is never broken, and the run ends with the data itself rather than an extra
//! extraction call. The data is checked here; a mismatch goes back to the planner to fix, and what
//! is printed always matches the schema or the run fails.

use serde_json::{Value, json};

/// Read `--schema`: inline JSON, or `@path` to a file.
pub fn parse(arg: &str) -> Result<Value, String> {
    let text = match arg.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path).map_err(|e| format!("--schema {path}: {e}"))?,
        None => arg.to_string(),
    };
    let schema: Value =
        serde_json::from_str(&text).map_err(|e| format!("--schema is not JSON: {e}"))?;
    if !schema.is_object() {
        return Err("--schema must be a JSON Schema object".into());
    }
    Ok(schema)
}

pub const TOOL: &str = "result";

/// The `result` tool: the user's schema as `data`, plus whether the goal was reached.
pub fn tool(schema: &Value) -> Value {
    json!({"type": "function", "function": {
        "name": TOOL,
        "description": "Finish the task: return what you found as `data`, matching its schema exactly (copy \
            numbers, names and dates from the pages; null only where the schema allows it and the page has no \
            value). Call it once, when done, instead of answering in text.",
        "parameters": {"type": "object", "required": ["data", "summary", "achieved"], "properties": {
            "data": schema,
            "summary": {"type": "string", "description": "One or two sentences for a person: what was found or done."},
            "achieved": {"type": "boolean", "description": "Whether the goal was reached."}
        }}
    }})
}

/// What the planner is told at the start, in the first message (part of the cached prefix).
pub const BRIEF: &str = "When you are done, call `result` with the data its schema asks for; gather every field it needs while you browse.";

/// A `result` call read: the data, the summary and whether the goal was reached, or what to fix.
pub fn take(args: &Value, schema: &Value) -> Result<(Value, String, bool), String> {
    let data = args.get("data").cloned().unwrap_or(Value::Null);
    let mut problems = vec![];
    violations(schema, &data, "data", &mut problems);
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    let summary = args
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let achieved = args
        .get("achieved")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    Ok((data, summary, achieved))
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
mod tests {
    use super::*;

    fn problems(schema: Value, value: Value) -> Vec<String> {
        let mut out = vec![];
        violations(&schema, &value, "", &mut out);
        out
    }

    #[test]
    fn checks_types_required_enums_and_nesting() {
        let schema = json!({"type": "object", "required": ["price", "date"], "properties": {
            "price": {"type": "number"}, "date": {"type": "string"},
            "stops": {"type": ["integer", "null"]}, "class": {"enum": ["economy", "business"]},
            "legs": {"type": "array", "items": {"type": "object", "required": ["from"], "properties": {"from": {"type": "string"}}}}
        }});
        assert!(problems(schema.clone(), json!({"price": 93, "date": "2026-11-27", "stops": null, "class": "economy", "legs": [{"from": "LCA"}]})).is_empty());
        assert_eq!(
            problems(
                schema,
                json!({"price": "93", "stops": 1.5, "class": "first", "legs": [{}]})
            ),
            [
                "the result is missing \"date\"",
                ".class must be one of [\"economy\",\"business\"]",
                ".legs[0] is missing \"from\"",
                ".price must be number",
                ".stops must be integer or null",
            ]
        );
    }

    #[test]
    fn reads_schemas_and_checks_result_calls() {
        assert!(parse("{\"type\":\"object\"}").is_ok());
        assert!(parse("[1]").is_err());
        assert!(parse("@/nonexistent/schema.json").is_err());

        let schema = json!({"type": "object", "required": ["price"], "properties": {"price": {"type": "number"}}});
        let call = json!({"data": {"price": 93}, "summary": "€93", "achieved": true});
        assert_eq!(
            take(&call, &schema),
            Ok((json!({"price": 93}), "€93".into(), true))
        );
        let wrong = json!({"data": {"price": "93"}, "summary": "", "achieved": true});
        assert_eq!(
            take(&wrong, &schema),
            Err("data.price must be number".into())
        );
    }
}
