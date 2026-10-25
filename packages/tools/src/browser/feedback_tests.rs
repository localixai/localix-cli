use serde_json::json;

use super::said;

#[test]
fn an_element_reads_with_its_note_selector_and_source() {
    let fb = json!({
        "n": 2,
        "url": "http://localhost:5173/",
        "note": "make it  red",
        "element": {
            "tag": "<button class=\"save\">",
            "role": "button",
            "text": "Save",
            "selector": "#form > button",
            "path": "form#form > button.save",
            "box": {"x": 10, "y": 20, "width": 80, "height": 30},
            "styles": {"color": "rgb(0, 0, 0)"},
            "source": {"file": "src/App.tsx", "line": 12, "column": 4, "components": ["Save", "App"]}
        }
    });
    let text = said(&fb).unwrap_or_default();
    assert!(text.starts_with("The person commented (#2) on http://localhost:5173/: make it red"));
    assert!(text.contains("Selector: #form > button"));
    assert!(text.contains("Box: 10,20 80×30"));
    assert!(text.contains("Styles: color: rgb(0, 0, 0)"));
    assert!(text.contains("Source: src/App.tsx:12:4 in Save < App"));
}

#[test]
fn an_empty_note_asks_to_look_and_a_region_lists_what_is_in_it() {
    let fb = json!({
        "n": 1, "url": "u", "note": "",
        "region": {"x": 0, "y": 0, "width": 5, "height": 6},
        "members": [{"selector": "h1", "text": "Title"}]
    });
    let text = said(&fb).unwrap_or_default();
    assert!(text.contains(": look at this"));
    assert!(text.contains("Region: 0,0 5×6\n- h1 Title"));
}

#[test]
fn a_long_field_is_cut_and_anything_else_is_nothing() {
    let fb = json!({"n": 1, "url": "u", "note": "x".repeat(5000), "region": {}});
    let text = said(&fb).unwrap_or_default();
    assert!(
        text.lines()
            .next()
            .is_some_and(|line| line.chars().count() < 2100)
    );
    assert_eq!(said(&json!({"note": "hi"})), None);
}

#[test]
fn a_picture_takes_the_box_with_room_around_it_kept_in_the_view() {
    use super::framed as clip;
    let fb = json!({"element": {"box": {"x": 10, "y": 100, "width": 80, "height": 30}}});
    let at = clip(&fb).unwrap();
    assert_eq!((at.x, at.y, at.width, at.height), (0.0, 76.0, 114.0, 78.0));
    assert!(clip(&json!({"region": {"x": 5, "y": 5, "width": 0, "height": 9}})).is_none());
}
