use contracts::hands::{Outcome, Report};
use serde_json::json;

use super::*;

#[test]
fn the_model_reads_the_outline_without_refs() {
    let outline =
        "- link \"Back\" [ref=back]\n- heading 2\n  - link \"Tbilisi\" [ref=e19]\n- text \"[ref=\"";
    assert_eq!(
        without_refs(outline),
        "- link \"Back\"\n- heading 2\n  - link \"Tbilisi\"\n- text \"[ref=\""
    );
}

#[test]
fn the_helper_report_lists_its_steps_and_the_values_it_never_typed() {
    let done = Report {
        steps: vec![
            "TYPE_TEXT combobox \"Where to?\" [e4] ← \"Tbilisi\" · 99%".into(),
            "DONE · 80%".into(),
        ],
        outcome: Outcome::Done { probability: 0.8 },
        view: None,
    };
    let values = ["Tbilisi".to_string(), "Larnaca".to_string()];
    assert_eq!(
        helper(&done, &values),
        "### Helper: done (80% sure)\n\
         1. TYPE_TEXT combobox \"Where to?\" [e4] ← \"Tbilisi\" · 99%\n\
         2. DONE · 80%\n\
         Values never typed: \"Larnaca\"."
    );
}

#[test]
fn arguments_are_typed_and_unknown_ones_refused() {
    let args: Args =
        serde_json::from_value(json!({"goal": "Search", "values": ["x"], "read": true})).unwrap();
    assert_eq!(
        (args.goal.as_deref(), args.values.len(), args.read),
        (Some("Search"), 1, true)
    );
    assert!(serde_json::from_value::<Args>(json!({"click": "e4"})).is_err());
    assert_eq!(given(&Some("  ".into())), None);
}
