use super::*;

#[expect(
    clippy::needless_pass_by_value,
    reason = "a test helper takes its arguments as the tests write them, by value"
)]
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
        take(&call, Some(&schema)),
        Ok(Finished {
            data: Some(json!({"price": 93})),
            summary: "€93".into(),
            achieved: true
        })
    );
    let wrong = json!({"data": {"price": "93"}, "summary": "", "achieved": true});
    assert_eq!(
        take(&wrong, Some(&schema)),
        Err(Refused::Mismatch("data.price must be number".into()))
    );
}

#[test]
fn fields_put_beside_achieved_instead_of_inside_data_are_the_data() {
    // The schema has a `summary` of its own, as a pull request's description does: the model
    // wrote its fields at the top of the call, next to `achieved`, with no `data`.
    let schema = json!({"type": "object", "required": ["summary", "risk"], "properties": {
        "summary": {"type": "string"}, "risk": {"enum": ["low", "medium", "high"]}
    }});
    let flat = json!({"summary": "Adds a cache.", "risk": "low", "achieved": true});
    assert_eq!(
        take(&flat, Some(&schema)),
        Ok(Finished {
            data: Some(json!({"summary": "Adds a cache.", "risk": "low"})),
            summary: String::new(),
            achieved: true
        })
    );
}

#[test]
fn a_call_with_no_data_says_where_the_fields_go() {
    let schema = json!({"type": "object", "required": ["price"], "properties": {"price": {"type": "number"}}});
    let call = json!({"summary": "€93", "achieved": true});
    let Err(Refused::Mismatch(error)) = take(&call, Some(&schema)) else {
        panic!("a schema mismatch");
    };
    assert!(error.contains("inside `data`"), "{error}");
}
