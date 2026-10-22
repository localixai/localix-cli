//! Localix Surf: a goal in, a native browser window driven to it, an answer out.
//!
//! A chat model plans ([`orchestrator`]) with one tool ([`tools`]); a decision model ([`decision`])
//! drives each sub-goal on the page ([`goal`]) through the system WebView ([`browser`]).

pub mod auth;
pub mod auth_page;
pub mod browser;
pub mod decision;
pub mod doctor;
pub mod exit;
pub mod expect;
pub mod goal;
pub mod goal_ask;
pub mod meter;
pub mod openrouter;
pub mod orchestrator;
pub mod planner;
pub mod report;
pub mod run;
pub mod skills;
pub mod start;
pub mod step;
pub mod structured;
pub mod tools;
pub mod values;
