//! How a task ended, as the report and the exit code say it, and what the decision model spent
//! on it.

use agent::agent::Status;
use serde_json::Value;

use crate::brake::Halt;
use crate::exit;

/// How a task ended, for the report and the exit code.
#[derive(Debug, PartialEq)]
pub(crate) struct Ended {
    pub status: &'static str,
    pub code: i32,
    pub error: Option<String>,
}

impl Ended {
    pub(crate) fn of(
        status: &Status,
        data: Option<&Result<Value, String>>,
        halt: Option<Halt>,
        timeout: u64,
    ) -> Self {
        let ended = |status, code, error: Option<String>| Self {
            status,
            code,
            error,
        };
        match (halt, status) {
            (Some(Halt::TimedOut), _) => ended(
                "exhausted",
                exit::EXHAUSTED,
                Some(format!(
                    "the time limit ran out ({timeout} s; raise it with --timeout)"
                )),
            ),
            (Some(Halt::Closed), _) => ended("interrupted", exit::OK, None),
            (Some(Halt::Interrupted), _) => ended("interrupted", exit::INTERRUPTED, None),
            (None, Status::Error(error)) => ended("error", exit::ENVIRONMENT, Some(error.clone())),
            (None, Status::Exhausted) => ended(
                "exhausted",
                exit::EXHAUSTED,
                Some("out of turns before an answer (raise them with --max-turns)".into()),
            ),
            (None, Status::NotAchieved) => ended("not_achieved", exit::FAILED, None),
            (None, _) if matches!(data, Some(Err(_))) => ended("not_achieved", exit::FAILED, None),
            (None, Status::Achieved) => ended("achieved", exit::OK, None),
            (None, Status::Unstated) => ended("unstated", exit::OK, None),
        }
    }
}
