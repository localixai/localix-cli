//! A tool's arguments as a TypeScript declaration: what the model reads to call the tool from a
//! script, `tools.<name>(args)`, with each argument's type, whether it may be left out, and what
//! it is for.

use std::fmt::Write;

use serde_json::Value;

/// `name(args: {...}): Promise<string>;`, its object indented one level past `depth`.
pub(super) fn declaration(name: &str, schema: &Value, depth: usize) -> String {
    format!("{name}(args: {}): Promise<string>;", shape(schema, depth))
}

/// A JSON Schema as a TypeScript type; `unknown` for what it does not say.
fn shape(schema: &Value, depth: usize) -> String {
    if let Some(options) = schema["enum"].as_array().filter(|all| !all.is_empty()) {
        let options: Vec<String> = options.iter().map(Value::to_string).collect();
        return options.join(" | ");
    }
    match schema["type"].as_str() {
        Some("string") => "string".into(),
        Some("integer" | "number") => "number".into(),
        Some("boolean") => "boolean".into(),
        Some("array") => format!("{}[]", shape(&schema["items"], depth)),
        Some("object") => object(schema, depth),
        _ => "unknown".into(),
    }
}

fn object(schema: &Value, depth: usize) -> String {
    let Some(properties) = schema["properties"]
        .as_object()
        .filter(|all| !all.is_empty())
    else {
        return "{}".into();
    };
    let required: Vec<&str> = (schema["required"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect();
    let inner = "  ".repeat(depth + 1);
    let mut out = String::from("{\n");
    for (key, property) in properties {
        if let Some(about) = property["description"].as_str() {
            let _ = writeln!(out, "{inner}/** {about} */");
        }
        let optional = if required.contains(&key.as_str()) {
            ""
        } else {
            "?"
        };
        let _ = writeln!(
            out,
            "{inner}{key}{optional}: {};",
            shape(property, depth + 1)
        );
    }
    let _ = write!(out, "{}}}", "  ".repeat(depth));
    out
}
