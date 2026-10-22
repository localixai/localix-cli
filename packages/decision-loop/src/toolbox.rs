//! The tools the loop may use, gathered at run time: whoever composes the loop adds them, and may add
//! or remove more while it runs. The loop reads the current set each time it picks a tool; it knows
//! no tool by name.

use std::sync::{Arc, RwLock};

use agent::tool::Tool;

/// A tool as the toolbox holds it: shareable across the threads that add, remove and drive tools.
pub type SharedTool = Arc<dyn Tool + Send + Sync>;

#[derive(Default)]
pub struct Toolbox {
    tools: RwLock<Vec<SharedTool>>,
}

impl Toolbox {
    /// Add `tool`, replacing one with the same name.
    pub fn add(&self, tool: SharedTool) {
        let mut tools = self.tools.write().unwrap();
        tools.retain(|known| known.name() != tool.name());
        tools.push(tool);
    }

    /// Remove the tool named `name`; whether there was one.
    pub fn remove(&self, name: &str) -> bool {
        let mut tools = self.tools.write().unwrap();
        let before = tools.len();
        tools.retain(|tool| tool.name() != name);
        tools.len() != before
    }

    pub fn get(&self, name: &str) -> Option<SharedTool> {
        self.tools
            .read()
            .unwrap()
            .iter()
            .find(|tool| tool.name() == name)
            .cloned()
    }

    /// The tools now, in the order they were added.
    pub fn tools(&self) -> Vec<SharedTool> {
        self.tools.read().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent::tool::{Action, Observation, ToolResult};

    struct Named(&'static str);

    impl Tool for Named {
        fn name(&self) -> &str {
            self.0
        }
        fn description(&self) -> &str {
            "a test tool"
        }
        fn start(&self, _: &str, _: Option<&str>) -> ToolResult<()> {
            Ok(())
        }
        fn observe(&self) -> ToolResult<Observation> {
            Ok(Observation::default())
        }
        fn act(&self, _: &str, _: &Action) -> ToolResult<()> {
            Ok(())
        }
    }

    #[test]
    fn tools_come_and_go_by_name() {
        let toolbox = Toolbox::default();
        toolbox.add(Arc::new(Named("a")));
        toolbox.add(Arc::new(Named("b")));
        toolbox.add(Arc::new(Named("a")));
        let names: Vec<String> = toolbox.tools().iter().map(|t| t.name().into()).collect();
        assert_eq!(names, ["b", "a"], "a second `a` replaces the first");
        assert!(toolbox.remove("b"));
        assert!(!toolbox.remove("b"));
        assert!(toolbox.get("a").is_some() && toolbox.get("b").is_none());
    }
}
