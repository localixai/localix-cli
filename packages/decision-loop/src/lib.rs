//! The decision loop: the hands that carry a short task out on a view with a decision model
//! alone. It works with any view (`contracts::view::View`) and any decision model
//! (`contracts::model::Decider`), and no particular one. [`decision_loop::DecisionLoop`] implements
//! `contracts::hands::Hands`: it resumes [`goal::GoalRun`] until the task is done; [`ask`] builds
//! each step's question.

mod ask;
mod choice;
pub mod decision_loop;
mod goal;
pub mod meter;
mod offer;
pub mod step;
