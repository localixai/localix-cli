use super::*;

#[test]
fn what_came_back_from_the_background_reads_as_where_it_came_from() {
    assert_eq!(
        shown("<monitor job=\"ticks\">\ntick 1\n</monitor>"),
        "≋ ticks · tick 1"
    );
    assert_eq!(
        shown("(scheduled `hello`) say hello"),
        "◷ hello · say hello"
    );
    assert_eq!(shown("fix the parser"), "fix the parser");
}

#[test]
fn a_monitor_batch_that_starts_a_task_reads_as_its_monitor_without_the_note_for_the_model() {
    let goal = format!(
        "{}<monitor job=\"ticks\">\ntick 2\n</monitor>",
        crate::jobs::FROM_MONITOR
    );
    assert_eq!(shown(&goal), "≋ ticks · tick 2");
}

#[test]
fn init_reads_as_the_command_the_person_typed() {
    assert_eq!(shown(crate::commands::extras::INIT), "/init");
}
