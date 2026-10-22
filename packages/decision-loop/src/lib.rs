//! The decision loop: a goal driven by a decision model through tools. It knows how to work with any
//! tool (the protocol in `agent::tool`) and any decision model (`agent::model::Decider`), and no
//! particular one; which tools exist is decided at run time ([`toolbox::Toolbox`]).
//! [`decision_loop::DecisionLoop`] picks a tool for the goal, starts it and resumes
//! [`goal::GoalRun`] until the goal is done; [`ask`] builds each step's question.

pub mod ask;
pub mod choice;
pub mod decision_loop;
pub mod expect;
pub mod goal;
pub mod meter;
pub mod step;
pub mod toolbox;
pub mod values;
