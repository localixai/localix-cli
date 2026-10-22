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
fn a_result_call_is_checked_against_its_schema() {
    let schema = json!({"type": "object", "required": ["price"], "properties": {"price": {"type": "number"}}});
    let call = json!({"data": {"price": 93}, "summary": "€93", "achieved": true});
    assert_eq!(
        take(&call, &schema),
        Ok(Finished {
            data: json!({"price": 93}),
            summary: "€93".into(),
            achieved: true
        })
    );
    let wrong = json!({"data": {"price": "93"}, "summary": "", "achieved": true});
    assert_eq!(
        take(&wrong, &schema),
        Err("data.price must be number".into())
    );
}
