use std::path::Path;

use super::*;
use crate::testing::Scratch;

fn failed(result: Result<PathBuf, ToolError>) -> String {
    match result {
        Err(ToolError::Failed(message)) => message,
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[test]
fn relative_paths_resolve_against_the_root_by_name() {
    let dir = Scratch::new();
    let workspace = Workspace::new(dir.path()).unwrap();
    let root = workspace.root().to_path_buf();
    assert_eq!(
        workspace.readable("src/./lib.rs").unwrap(),
        root.join("src/lib.rs")
    );
    assert_eq!(
        workspace.readable("src/../Cargo.toml").unwrap(),
        root.join("Cargo.toml")
    );
    assert_eq!(workspace.readable("  a.txt ").unwrap(), root.join("a.txt"));
}

#[test]
fn an_empty_path_is_refused() {
    let dir = Scratch::new();
    let workspace = Workspace::new(dir.path()).unwrap();
    assert_eq!(failed(workspace.readable("  ")), "the path is empty");
}

#[test]
fn absolute_paths_are_readable_anywhere() {
    let dir = Scratch::new();
    let workspace = Workspace::new(dir.path().join(".")).unwrap();
    let outside = std::env::temp_dir().join("elsewhere.txt");
    assert_eq!(
        workspace.readable(outside.to_str().unwrap()).unwrap(),
        outside
    );
}

#[test]
fn reports_show_paths_relative_to_the_root() {
    let dir = Scratch::new();
    let workspace = Workspace::new(dir.path()).unwrap();
    let root = workspace.root().to_path_buf();
    assert_eq!(workspace.shown(&root), ".");
    assert_eq!(
        workspace.shown(&root.join("src").join("lib.rs")),
        Path::new("src").join("lib.rs").display().to_string()
    );
    let outside = std::env::temp_dir().join("x.txt");
    assert_eq!(workspace.shown(&outside), outside.display().to_string());
}

#[test]
fn a_denied_path_is_refused_however_it_is_reached() {
    let dir = Scratch::new();
    let secret = Scratch::new();
    let key = secret.file("llx/credentials", "sk-or-secret");
    let sessions = secret.path().join("sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    let workspace = Workspace::new(dir.path())
        .unwrap()
        .denying([key.clone(), sessions.clone()]);
    for path in [&key, &sessions.join("1-ab.json")] {
        let message = failed(workspace.readable(path.to_str().unwrap()));
        assert!(message.ends_with("is off limits to the tools"), "{message}");
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&key, dir.path().join("innocent.txt")).unwrap();
        assert!(failed(workspace.readable("innocent.txt")).contains("off limits"));
    }
    assert!(workspace.readable("anything-else.txt").is_ok());
}

#[test]
fn verbatim_paths_become_plain_when_they_mean_the_same() {
    assert_eq!(
        unverbatim(r"\\?\C:\Users\me\repo").as_deref(),
        Some(r"C:\Users\me\repo")
    );
    assert_eq!(unverbatim(r"\\?\D:\").as_deref(), Some(r"D:\"));
    assert_eq!(
        unverbatim(r"\\?\UNC\server\share\repo").as_deref(),
        Some(r"\\server\share\repo")
    );
    assert_eq!(unverbatim(r"C:\already\plain"), None);
    assert_eq!(unverbatim(r"\\?\C:\dir\NUL"), None, "a device name");
    assert_eq!(unverbatim(r"\\?\C:\dir\com1.txt"), None, "a device name");
    assert_eq!(unverbatim(r"\\?\C:\dir\trailing."), None);
    let long = format!(r"\\?\C:\{}", "a".repeat(300));
    assert_eq!(unverbatim(&long), None, "past MAX_PATH");
    assert_eq!(unverbatim(r"\\?\Volume{1234}\x"), None);
}

#[cfg(windows)]
#[test]
fn the_root_is_never_verbatim_on_windows() {
    let dir = Scratch::new();
    let workspace = Workspace::new(dir.path()).unwrap();
    assert!(
        !workspace.root().to_string_lossy().starts_with(r"\\?\"),
        "{}",
        workspace.root().display()
    );
}
