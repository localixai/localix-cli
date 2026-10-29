//! How a task ended, as the report and the exit code say it.

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

/// The limits a task ran within, for the words of the one it hit.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Bounds {
    /// Seconds.
    pub timeout: u64,
    /// Dollars.
    pub budget: Option<f64>,
}

impl Ended {
    pub(crate) fn of(
        status: &Status,
        data: Option<&Result<Value, String>>,
        halt: Option<Halt>,
        bounds: Bounds,
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
                    "the time limit ran out ({} s; raise it with --timeout)",
                    bounds.timeout
                )),
            ),
            (Some(Halt::OverBudget), _) => ended(
                "exhausted",
                exit::EXHAUSTED,
                Some(format!(
                    "the budget ran out (${}; raise it with --budget)",
                    bounds.budget.unwrap_or_default()
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
        }
    }
}

#[cfg(test)]
#[path = "ended_tests.rs"]
mod tests;
