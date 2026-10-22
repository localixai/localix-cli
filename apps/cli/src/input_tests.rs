use super::*;
use crate::testing::Scratch;

fn one(text: &str) -> Result<Prompt, Failure> {
    Ok(Prompt::One(text.into()))
}

#[test]
fn a_prompt_alone_is_the_task() {
    assert_eq!(assemble(Some("  fix it "), None), one("fix it"));
    assert!(assemble(Some("   "), None).is_err());
}

#[test]
fn piped_stdin_is_added_to_a_prompt() {
    assert_eq!(
        assemble(Some("review this"), Some("diff --git a b\n")),
        one("review this\n\n<stdin>\ndiff --git a b\n</stdin>")
    );
    assert_eq!(
        assemble(Some("review this"), Some("  \n")),
        one("review this")
    );
}

#[test]
fn a_dash_or_no_prompt_reads_the_task_from_stdin() {
    assert_eq!(assemble(Some("-"), Some("the task\n")), one("the task"));
    assert_eq!(assemble(None, Some("the task")), one("the task"));
    assert!(matches!(
        assemble(Some("-"), Some("")),
        Err(Failure::Usage(_))
    ));
    assert!(matches!(
        assemble(None, Some(" \n")),
        Err(Failure::Usage(_))
    ));
}

#[test]
fn no_prompt_at_a_terminal_is_the_prompt_line() {
    assert_eq!(assemble(None, None), Ok(Prompt::Line));
}

#[test]
fn a_schema_is_inline_json_or_a_file() {
    assert!(schema("{\"type\":\"object\"}").is_ok());
    assert!(matches!(schema("[1]"), Err(Failure::Usage(_))));
    assert!(matches!(schema("{nope"), Err(Failure::Usage(_))));
    assert!(schema("@/nonexistent/schema.json").is_err());
    let scratch = Scratch::new();
    let path = scratch.file("schema.json", "{\"type\":\"object\"}");
    assert!(schema(&format!("@{}", path.display())).is_ok());
}

#[test]
fn reading_stdin_is_said_where_it_could_wait_unseen() {
    assert!(
        waiting(Some("fix it"), Stdin::Stream)
            .unwrap()
            .contains("</dev/null")
    );
    assert_eq!(
        waiting(Some("fix it"), Stdin::File),
        None,
        "a file or /dev/null always ends"
    );
    assert!(
        waiting(Some("-"), Stdin::Terminal)
            .unwrap()
            .contains("Ctrl-D")
    );
    assert_eq!(
        waiting(Some("-"), Stdin::Stream),
        None,
        "a pipe given on purpose"
    );
    assert_eq!(waiting(None, Stdin::Stream), None);
    assert_eq!(
        waiting(Some("fix it"), Stdin::Terminal),
        None,
        "a terminal is not read"
    );
}
