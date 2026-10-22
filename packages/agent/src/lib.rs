//! The agent: a chat model reaches a goal through tools and answers. It works with any chat model
//! (`contracts::chat::Chat`) and any tools (`contracts::tool::Tool`), and no particular one: what
//! a tool's world is like comes in its spec, and where the run happens in the app's context.
//! - [`agent`]: the loop, its stop conditions and what a run spent.
//! - [`observer`]: what a run shows as it goes (calls, results, turns).
//! - [`conversation`]: the messages, and dropping older states past a budget.
//! - [`schema`]: `--schema`, the answer as data through the `result` tool.
//! - [`answer`]: the final reply and its `RESULT:` line.

pub mod agent;
pub mod answer;
pub mod conversation;
pub mod observer;
pub mod schema;

#[cfg(test)]
mod fakes;
