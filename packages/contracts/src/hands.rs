//! The hands port: a short task carried out on a view by whoever drives it (a decision model that
//! clicks and types), and how it went. A tool hands its view over without knowing who drives it.

use crate::view::{Observation, View};

/// One task: what to reach on the view, and the only strings that may be typed into it.
#[derive(Debug, Clone, Copy)]
pub struct Task<'a> {
    pub goal: &'a str,
    pub values: &'a [String],
}

/// How a task ended.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// The hands judged the task done; how sure they were.
    Done { probability: f64 },
    /// Nothing offered could make progress.
    Blocked,
    /// The action budget ran out first.
    Exhausted,
    /// Whoever decides could not be reached: asking again the same way will not help.
    Unreachable(String),
    /// The run stopped on a guard (a repeated action, a loop, a failed action): why.
    Stopped(String),
}

/// What the hands did, step by step, and the view they left.
#[derive(Debug, Clone)]
pub struct Report {
    /// Each step as one line: `CLICK button "Search" · 91%`.
    pub steps: Vec<String>,
    pub outcome: Outcome,
    /// The view once it settled, when it could be read.
    pub view: Option<Observation>,
}

pub trait Hands: Send + Sync {
    fn carry_out(&self, view: &dyn View, task: Task) -> Report;
}
