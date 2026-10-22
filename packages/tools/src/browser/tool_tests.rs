use super::*;
use contracts::hands::{Outcome, Report};

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
