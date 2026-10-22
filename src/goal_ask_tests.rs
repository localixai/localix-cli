use super::*;
use crate::browser::{Scroll, SelectOption, Size};

fn element(reference: &str, role: &str, name: &str) -> Element {
    Element {
        reference: reference.into(),
        role: role.into(),
        name: name.into(),
        value: None,
        states: vec![],
        editable: role == "textbox",
        dialog: false,
        options: vec![],
    }
}

fn page(elements: Vec<Element>) -> Observation {
    Observation {
        epoch: "1".into(),
        url: "http://x".into(),
        title: "T".into(),
        text: "Book a trip".into(),
        viewport: Size { w: 1, h: 1 },
        scroll: Scroll { y: 0, height: 1 },
        elements,
        omitted: 0,
        busy: false,
        fields: vec![],
    }
}

fn choice(choice: &str) -> Answer {
    Answer::Choice {
        choice: choice.into(),
        probabilities: [(choice.to_string(), 1.0)].into(),
        confidence: 1.0,
    }
}

#[test]
fn offers_only_what_each_operation_can_act_on() {
    let mut disabled = element("e3", "button", "Later");
    disabled.states.push("disabled".into());
    let mut class = element("e4", "combobox", "Class");
    class.options = vec![SelectOption {
        value: "b".into(),
        label: "Business".into(),
    }];
    let page = page(vec![
        element("e1", "textbox", "City"),
        element("e2", "button", "Search"),
        disabled,
        class,
    ]);
    let values = vec!["Zurich".to_string()];
    let ask = ask(&AskInput {
        goal: "g",
        page: &page,
        values: &values,
        history: &[],
    });
    let keys = |head: &str| ask.offered[head].keys().cloned().collect::<Vec<_>>();
    assert_eq!(keys("click_target"), ["e1", "e2"]);
    assert_eq!(keys("type_entry"), ["e1=v1"]);
    assert_eq!(keys("select_target"), ["e4:1"]);
    assert!(
        !ask.offered["operation"].contains_key("PRESS_ENTER"),
        "Enter only right after typing"
    );

    let answers = [
        ("operation".into(), choice("TYPE_TEXT")),
        ("type_entry".into(), choice("e1=v1")),
    ]
    .into();
    let pick = ask.read(&answers).unwrap();
    assert_eq!(
        (pick.reference.as_deref(), pick.value.as_deref()),
        (Some("e1"), Some("Zurich"))
    );
    let answers = [
        ("operation".into(), choice("SELECT")),
        ("select_target".into(), choice("e4:1")),
    ]
    .into();
    let pick = ask.read(&answers).unwrap();
    assert_eq!(
        (pick.reference.as_deref(), pick.value.as_deref()),
        (Some("e4"), Some("b"))
    );
    let answers = [
        ("operation".into(), choice("CLICK")),
        ("click_target".into(), choice("e3")),
    ]
    .into();
    assert!(
        ask.read(&answers).is_err(),
        "a disabled control is never a target"
    );
}

#[test]
fn typing_offers_whole_field_value_pairs_and_skips_what_is_already_there() {
    let mut from = element("e1", "textbox", "Where from?");
    from.value = Some("Larnaca".into());
    let page = page(vec![from, element("e2", "textbox", "Where to?")]);
    let values = vec!["Larnaca".to_string(), "Tbilisi".to_string()];
    let ask = ask(&AskInput {
        goal: "g",
        page: &page,
        values: &values,
        history: &[],
    });
    let offered: Vec<&Value> = ask.offered["type_entry"].values().collect();
    assert_eq!(
        offered,
        [
            &json!("textbox \"Where from?\" = \"Larnaca\" ← \"Tbilisi\""),
            &json!("textbox \"Where to?\" ← \"Larnaca\""),
            &json!("textbox \"Where to?\" ← \"Tbilisi\""),
        ]
    );
    let answers = [
        ("operation".into(), choice("TYPE_TEXT")),
        ("type_entry".into(), choice("e2=v2")),
    ]
    .into();
    let pick = ask.read(&answers).unwrap();
    assert_eq!(
        (pick.reference.as_deref(), pick.value.as_deref()),
        (Some("e2"), Some("Tbilisi"))
    );
}

#[test]
fn a_dialog_scopes_the_offer_and_scrolling_stops_when_it_stops_moving() {
    let mut accept = element("e9", "button", "Accept");
    accept.dialog = true;
    let page = page(vec![element("e1", "button", "Search"), accept]);
    let still = GoalStep {
        operation: Operation::ScrollDown,
        reference: None,
        element: None,
        value: None,
        probability: 1.0,
        changed: Some(false),
    };
    let ask = ask(&AskInput {
        goal: "g",
        page: &page,
        values: &[],
        history: std::slice::from_ref(&still),
    });
    assert_eq!(
        ask.offered["click_target"].keys().collect::<Vec<_>>(),
        ["e9"]
    );
    assert_eq!(ask.state["dialog_open"], json!(true));
    assert!(!ask.offered["operation"].contains_key("SCROLL_DOWN"));
    assert!(ask.offered["operation"].contains_key("SCROLL_UP"));
    let idle = GoalStep {
        operation: Operation::Wait,
        ..still
    };
    let ask = super::ask(&AskInput {
        goal: "g",
        page: &page,
        values: &[],
        history: &[idle],
    });
    assert!(
        !ask.offered["operation"].contains_key("WAIT"),
        "a stable page is not waited on again"
    );
}
