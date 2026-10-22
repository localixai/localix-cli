use contracts::history::Turn;

use super::*;

fn session(goals: &[&str]) -> Session {
    Session {
        id: "1700000000-ab12".into(),
        updated: session::now(),
        turns: goals
            .iter()
            .map(|goal| Turn {
                goal: goal.to_string(),
                answer: String::new(),
                status: "achieved".into(),
                at: 0,
            })
            .collect(),
        ..Session::default()
    }
}

#[test]
fn a_row_shows_the_id_the_count_and_the_first_task() {
    let row = row(&session(&["fix the\n parser", "add a test"]));
    assert!(row.starts_with("1700000000-ab12  "));
    assert!(row.contains("just now"));
    assert!(row.contains(" 2 tasks  ?  fix the parser\n"), "{row}");
    assert!(super::row(&session(&["x"])).contains("1 task "));
}

#[test]
fn a_long_first_task_is_cut() {
    let long = "a".repeat(TASK_CHARS + 5);
    let row = row(&session(&[&long]));
    assert!(row.contains(&format!("{}…", "a".repeat(TASK_CHARS))));
}

#[test]
fn a_row_shows_where_the_session_ran_and_escapes_what_was_typed() {
    let mut ran = session(&["fix\u{1b}[2J it"]);
    ran.workspace = "/work/site".into();
    let row = row(&ran);
    assert!(row.contains("  /work/site  fix\\u{1b}[2J it\n"), "{row}");
}
