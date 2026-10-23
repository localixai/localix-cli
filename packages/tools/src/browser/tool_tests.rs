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
