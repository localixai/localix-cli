//! The agent: a chat model reaches a goal through tools and answers. It works with any chat model
//! (`contracts::chat::Chat`) and any tools (`contracts::tool::Tool`), and no particular one: what
//! a tool's world is like comes in its spec, and where the run happens in the app's context.
//! - [`agent`]: the loop and its stop conditions.
//! - [`history`]: how the conversation is fitted to the model's window before each request.
//! - [`observer`]: what a run shows as it goes (calls, results, turns).
//! - [`conversation`]: the messages, and dropping older states past a budget.
//! - [`schema`]: the `result` tool that ends every run, with `--schema`'s data.

pub mod agent;
mod calls;
pub mod conversation;
pub mod history;
pub mod observer;
mod schema;

#[cfg(test)]
mod fakes;
