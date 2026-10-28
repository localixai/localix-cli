use serde_json::json;
use tools::fs::PatchTool;

use super::*;
use crate::testing::Scratch;

fn edit(tool: &Tracked, path: &str, old: &str, new: &str) {
    tool.call(&json!(crate::testing::patch(path, old, new)))
        .unwrap();
}

#[test]
fn a_task_keeps_each_file_as_it_found_it_and_a_rewind_puts_them_back() {
    let scratch = Scratch::new();
    scratch.file("kept.txt", "one\n");
    let workspace = Workspace::new(scratch.path()).unwrap();
    let inner = PatchTool {
        workspace: &workspace,
        gate: &contracts::gate::Open,
    };
    let checkpoints = Checkpoints::default();
    let tracked = Tracked {
        inner: &inner,
        workspace: &workspace,
        checkpoints: &checkpoints,
    };
    checkpoints.begin("first", 3);
    edit(&tracked, "kept.txt", "one", "two");
    edit(&tracked, "kept.txt", "two", "three");
    checkpoints.begin("second", 7);
    edit(&tracked, "kept.txt", "three", "four");
    edit(&tracked, "new.txt", "", "made\n");
    let tasks = checkpoints.tasks();
    assert_eq!(
        (tasks[0].changed(), tasks[1].changed()),
        (1, 2),
        "first changes only"
    );
    let read = |name: &str| std::fs::read_to_string(scratch.path().join(name)).ok();
    // Back to before the second task: its file goes, the other is as the first left it.
    assert_eq!(checkpoints.restore(1), (2, vec![]));
    assert_eq!(read("kept.txt").as_deref(), Some("three\n"));
    assert_eq!(read("new.txt"), None);
    // Back to before the first: as it was before any task.
    assert_eq!(checkpoints.restore(0).0, 1);
    assert_eq!(read("kept.txt").as_deref(), Some("one\n"));
    assert!(checkpoints.tasks().is_empty());
    // Forgetting a task drops the way back, not what it did to the files.
    checkpoints.begin("third", 9);
    edit(&tracked, "kept.txt", "one", "five");
    checkpoints.begin("fourth", 11);
    checkpoints.forget(1);
    assert_eq!(checkpoints.tasks().len(), 1);
    assert_eq!(read("kept.txt").as_deref(), Some("five\n"));
}

#[test]
fn a_patch_keeps_every_file_it_names_including_where_a_file_moves_to() {
    let scratch = Scratch::new();
    scratch.file("a.txt", "one\n");
    scratch.file("b.txt", "bee\n");
    let workspace = Workspace::new(scratch.path()).unwrap();
    let inner = PatchTool {
        workspace: &workspace,
        gate: &contracts::gate::Open,
    };
    let checkpoints = Checkpoints::default();
    let tracked = Tracked {
        inner: &inner,
        workspace: &workspace,
        checkpoints: &checkpoints,
    };
    checkpoints.begin("patch", 0);
    let patch = "*** Begin Patch\n*** Update File: a.txt\n@@\n-one\n+two\n\
                 *** Add File: c.txt\n+new\n*** Update File: b.txt\n*** Move to: d.txt\n\
                 *** End Patch";
    tracked.call(&json!(patch)).unwrap();
    assert_eq!(checkpoints.tasks()[0].changed(), 4);
    assert_eq!(checkpoints.restore(0), (4, vec![]));
    let read = |name: &str| std::fs::read_to_string(scratch.path().join(name)).ok();
    assert_eq!(read("a.txt").as_deref(), Some("one\n"));
    assert_eq!(read("b.txt").as_deref(), Some("bee\n"));
    assert_eq!((read("c.txt"), read("d.txt")), (None, None));
}
