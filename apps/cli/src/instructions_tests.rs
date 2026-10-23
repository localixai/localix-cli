use std::collections::HashSet;
use std::sync::Mutex;

use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};

use super::*;
use crate::testing::Scratch;

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[test]
fn each_directory_gives_its_first_file_from_the_root_down() {
    let repo = Scratch::new();
    let root = repo.path();
    std::fs::create_dir_all(root.join(".git")).unwrap();
    write(&root.join("AGENTS.md"), "root rules");
    write(&root.join("CLAUDE.md"), "@AGENTS.md");
    write(&root.join("app/CLAUDE.md"), "app rules");
    write(&root.join("app/web/AGENTS.md"), "web rules");
    write(&root.join("app/web/AGENTS.override.md"), "web override");
    let found = project(&root.join("app/web"), TOTAL_LIMIT);
    let texts: Vec<&str> = found.iter().map(|f| f.text.as_str()).collect();
    assert_eq!(texts, ["root rules", "app rules", "web override"]);
}

#[test]
fn a_line_that_is_only_an_at_path_brings_that_file_in_once() {
    let repo = Scratch::new();
    let root = repo.path();
    write(
        &root.join("CLAUDE.md"),
        "Read these:\n@docs/style.md\n@docs/style.md\n@missing.md\nand email me@example.com",
    );
    write(&root.join("docs/style.md"), "Use tabs.\n@../CLAUDE.md");
    let found = in_dir(root, FILE_LIMIT).unwrap();
    assert_eq!(
        found.text,
        "Read these:\nUse tabs.\n@missing.md\nand email me@example.com"
    );
}

#[test]
fn the_persons_own_file_lives_in_the_llx_home() {
    let home = Scratch::new();
    assert_eq!(personal(Some(home.path())), None);
    write(&home.path().join(".llx/AGENTS.md"), "Answer briefly.");
    assert_eq!(personal(Some(home.path())).unwrap().text, "Answer briefly.");
}

#[test]
fn a_huge_file_is_cut_and_says_so() {
    let repo = Scratch::new();
    write(&repo.path().join("AGENTS.md"), &"x".repeat(FILE_LIMIT + 10));
    let found = project(repo.path(), TOTAL_LIMIT);
    assert!(
        found[0]
            .text
            .ends_with("[10 more bytes not shown: read the file for the rest]")
    );
}

struct Echo;

impl Tool for Echo {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read".into(),
            description: String::new(),
            parameters: json!({}),
        }
    }
    fn call(&self, _: &Value) -> Result<ToolOutput, ToolError> {
        Ok(ToolOutput {
            report: "contents".into(),
            state: None,
            image: None,
        })
    }
}

#[test]
fn a_subdirectorys_rules_come_once_with_the_first_file_touched_there() {
    let repo = Scratch::new();
    let root = repo.path().to_path_buf();
    write(&root.join("AGENTS.md"), "root rules");
    write(&root.join("crates/net/AGENTS.md"), "net rules");
    write(&root.join("crates/net/src/lib.rs"), "");
    let seen = Mutex::new(HashSet::from([root.clone()]));
    let tool = Instructed {
        inner: &Echo,
        root: root.clone(),
        seen: &seen,
    };
    let first = tool
        .call(&json!({"path": "crates/net/src/lib.rs"}))
        .unwrap()
        .report;
    assert!(
        first.starts_with("contents\n\n<project_instructions path="),
        "{first}"
    );
    assert!(first.contains("net rules") && first.contains("applies_to="));
    assert!(
        !first.contains("root rules"),
        "the context has those already"
    );
    let again = tool
        .call(&json!({"path": root.join("crates/net/src/lib.rs").display().to_string()}))
        .unwrap();
    assert_eq!(again.report, "contents");
    let outside = tool.call(&json!({"path": "/etc/hosts"})).unwrap();
    assert_eq!(outside.report, "contents");
}
