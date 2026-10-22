use super::*;

#[test]
fn the_last_statement_after_a_block_is_returned_too() {
    let forms = script_forms("let n = 0; while (n < 3) { n++ } n * 2");
    assert!(
        forms
            .iter()
            .any(|form| form.contains("while (n < 3) { n++ }\nreturn (n * 2\n)")),
        "{forms:#?}"
    );
    assert!(
        forms.last().unwrap().contains("{ n++ } n * 2\n}"),
        "the body as it is, last"
    );
}

#[test]
fn an_object_literal_after_a_block_still_finds_the_block() {
    let forms = script_forms("let n = 0; while (n < 3) { n++ } JSON.stringify({ n })");
    assert!(
        forms
            .iter()
            .any(|form| form.contains("{ n++ }\nreturn (JSON.stringify({ n })\n)")),
        "{forms:#?}"
    );
}
