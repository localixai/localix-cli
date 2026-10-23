use super::*;
use contracts::view::SelectOption;

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

fn view(elements: Vec<Element>) -> Observation {
    Observation {
        outline: String::new(),
        epoch: "1".into(),
        location: "http://x".into(),
        title: "T".into(),
        text: "Book a trip".into(),
        scroll: 0,
        elements,
        omitted: 0,
        busy: false,
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
    let view = view(vec![
        element("e1", "textbox", "City"),
        element("e2", "button", "Search"),
        disabled,
        class,
    ]);
    let values = vec!["Zurich".to_string()];
    let ask = ask(&AskInput {
        goal: "g",
        conventions: "",
        view: &view,
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
    let view = view(vec![from, element("e2", "textbox", "Where to?")]);
    let values = vec!["Larnaca".to_string(), "Tbilisi".to_string()];
    let ask = ask(&AskInput {
        goal: "g",
        conventions: "",
        view: &view,
        values: &values,
        history: &[],
    });
    let offered: Vec<&Value> = ask.offered["type_entry"].values().collect();
    // "Where from?" holds a value: filled. "Larnaca" is in a field: placed. Neither is offered.
    assert_eq!(offered, [&json!("textbox \"Where to?\" ← \"Tbilisi\"")]);
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
    let view = view(vec![element("e1", "button", "Search"), accept]);
    let still = GoalStep {
        operation: Operation::ScrollDown,
        reference: None,
        element: None,
        value: None,
        probability: 1.0,
        changed: Some(false),
        failed: None,
        new_view: false,
    };
    let ask = ask(&AskInput {
        goal: "g",
        conventions: "",
        view: &view,
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
        conventions: "",
        view: &view,
        values: &[],
        history: &[idle],
    });
    assert!(
        !ask.offered["operation"].contains_key("WAIT"),
        "a stable view is not waited on again"
    );
}

#[test]
fn a_settled_view_offers_no_wait_after_a_step_that_showed_something() {
    let next = GoalStep {
        operation: Operation::Click,
        reference: Some("e1".into()),
        element: Some("button \"Next\"".into()),
        value: None,
        probability: 1.0,
        changed: Some(true),
        failed: None,
        new_view: true,
    };
    let offered = |history: &[GoalStep], busy: bool| {
        let mut view = view(vec![element("e1", "button", "Next")]);
        view.busy = busy;
        let ask = ask(&AskInput {
            goal: "Click Next and wait for the next question",
            conventions: "",
            view: &view,
            values: &[],
            history,
        });
        ask.offered["operation"].contains_key("WAIT")
    };
    assert!(
        !offered(std::slice::from_ref(&next), false),
        "the next question is there"
    );
    assert!(
        offered(std::slice::from_ref(&next), true),
        "the view says it is working"
    );
    let silent = GoalStep {
        changed: Some(false),
        new_view: false,
        ..next
    };
    assert!(
        offered(&[silent], false),
        "a click that has shown nothing yet"
    );
}

#[test]
fn the_model_chooses_how_long_to_wait() {
    let mut view = view(vec![element("e1", "button", "Search")]);
    view.busy = true;
    let ask = ask(&AskInput {
        goal: "Search and wait for the results",
        conventions: "",
        view: &view,
        values: &[],
        history: &[],
    });
    assert!(ask.offered["wait_for"].contains_key("10"));
    let answers = [
        ("operation".into(), choice("WAIT")),
        ("wait_for".into(), choice("10")),
    ]
    .into();
    let pick = ask.read(&answers).unwrap();
    assert_eq!(pick.operation, Operation::Wait);
    assert_eq!(pick.value.as_deref(), Some("10"));
}

#[test]
fn a_target_refused_as_covered_is_not_offered_again_until_the_view_changes() {
    let view = view(vec![
        element("e1", "button", "New Game"),
        element("e2", "button", "Save"),
    ]);
    let refused = GoalStep {
        operation: Operation::Click,
        reference: Some("e1".into()),
        element: None,
        value: None,
        probability: 0.9,
        changed: Some(false),
        failed: Some("covered: a dialog \"Consent\" is on top; answer or close it first".into()),
        new_view: false,
    };
    let offered = |history: &[GoalStep]| {
        let ask = ask(&AskInput {
            goal: "g",
            conventions: "",
            view: &view,
            values: &[],
            history,
        });
        ask.offered["click_target"]
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(offered(std::slice::from_ref(&refused)), ["e2"]);
    let moved = GoalStep {
        reference: Some("e2".into()),
        changed: Some(true),
        failed: None,
        ..refused.clone()
    };
    assert_eq!(
        offered(&[refused, moved]),
        ["e1", "e2"],
        "the view changed since"
    );
}
