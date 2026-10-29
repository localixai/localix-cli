use agent::agent::Status;
use serde_json::json;

use super::*;
use crate::exit;

#[test]
fn an_answer_ends_with_the_code_its_result_line_says() {
    let ended = |status: Status| Ended::of(&status, None, None, 60);
    assert_eq!(ended(Status::Achieved).code, exit::OK);
    assert_eq!(ended(Status::NotAchieved).code, exit::FAILED);
    assert_eq!(ended(Status::NotAchieved).status, "not_achieved");
    let exhausted = ended(Status::Exhausted);
    assert_eq!(exhausted.code, exit::EXHAUSTED);
    assert!(exhausted.error.unwrap().contains("--max-turns"));
    let error = ended(Status::Error("no network".into()));
    assert_eq!(
        error,
        Ended {
            status: "error",
            code: exit::ENVIRONMENT,
            error: Some("no network".into())
        }
    );
}

#[test]
fn data_that_does_not_match_fails_the_run() {
    let mismatch = Err("the result did not match the schema".to_string());
    let ended = Ended::of(&Status::Achieved, Some(&mismatch), None, 60);
    assert_eq!(ended.code, exit::FAILED);
    let data = Ok(json!({"n": 1}));
    assert_eq!(
        Ended::of(&Status::Achieved, Some(&data), None, 60).code,
        exit::OK
    );
}

#[test]
fn a_halt_decides_the_status_and_exit_code() {
    // Whatever the model said, the halt decides.
    let model_unreachable = Status::Error("the time limit ran out".into());
    let stopped = Status::Error("the person stopped the task".into());
    for (status, halt, ended, code, error) in [
        (
            &model_unreachable,
            Halt::TimedOut,
            "exhausted",
            exit::EXHAUSTED,
            Some("90 s"),
        ),
        (
            &model_unreachable,
            Halt::Closed,
            "interrupted",
            exit::OK,
            None,
        ),
        (
            &stopped,
            Halt::Interrupted,
            "interrupted",
            exit::INTERRUPTED,
            None,
        ),
    ] {
        let got = Ended::of(status, None, Some(halt), 90);
        assert_eq!((got.status, got.code), (ended, code), "{halt:?}");
        match error {
            Some(part) => assert!(got.error.unwrap().contains(part), "{halt:?}"),
            None => assert_eq!(got.error, None, "{halt:?}"),
        }
    }
}
