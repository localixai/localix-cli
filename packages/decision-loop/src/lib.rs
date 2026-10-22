//! The decision loop: the hands that carry a short task out on a view with a decision model
//! alone. It works with any view (`contracts::view::View`) and any decision model
//! (`contracts::model::Decider`), and no particular one. [`decision_loop::DecisionLoop`] implements
//! `contracts::hands::Hands`: it resumes [`goal::GoalRun`] until the task is done; [`ask`] builds
//! each step's question.

pub mod ask;
pub mod choice;
pub mod decision_loop;
pub mod goal;
pub mod meter;
pub mod step;
