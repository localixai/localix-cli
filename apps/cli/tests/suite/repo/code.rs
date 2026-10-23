//! The code rules (ARCHITECTURE.md, S4 and S5), checked on the manifests and sources.

use std::path::Path;

use super::{root, walk};

/// Every crate's directory: the packages, the providers a level down, the apps.
fn crates() -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    for group in ["packages", "packages/providers", "apps"] {
        for entry in std::fs::read_dir(root().join(group)).unwrap().flatten() {
            if entry.path().join("Cargo.toml").is_file() {
                out.push(entry.path());
            }
        }
    }
    out
}

/// S4: one set of lints for the workspace, taken by every crate.
#[test]
fn every_crate_takes_the_workspace_lints() {
    let found = crates();
    assert!(found.len() > 8, "found only {} crates", found.len());
    let missing: Vec<String> = found
        .iter()
        .filter(|dir| {
            let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
            !manifest.contains("[lints]\nworkspace = true")
        })
        .map(|dir| dir.strip_prefix(root()).unwrap().display().to_string())
        .collect();
    assert!(
        missing.is_empty(),
        "add `[lints] workspace = true` to {missing:?}"
    );
}

/// The crates that call no OS directly: the vocabulary, the core, the stores, the providers.
const SAFE: [&str; 8] = [
    "packages/client",
    "packages/contracts",
    "packages/protocol",
    "packages/agent",
    "packages/decision-loop",
    "packages/queue",
    "packages/session",
    "packages/providers",
];

/// S5: `unsafe` lives only where the OS is called, in the tools and the apps (tests aside).
#[test]
fn unsafe_lives_only_where_the_os_is_called() {
    let mut found = vec![];
    for dir in SAFE {
        let mut files = vec![];
        // Working code: a test may set the environment, which Rust marks unsafe.
        let rust = |path: &Path| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.ends_with(".rs")
                && !name.ends_with("_tests.rs")
                && !path.starts_with(root().join(dir).join("tests"))
        };
        walk(&root().join(dir), &rust, &mut files);
        assert!(!files.is_empty(), "{dir} has no sources");
        for path in files {
            let text = std::fs::read_to_string(&path).unwrap();
            let code = text.lines().filter(|line| !line.trim().starts_with("//"));
            if code.into_iter().any(|line| line.contains("unsafe ")) {
                found.push(path.strip_prefix(root()).unwrap().display().to_string());
            }
        }
    }
    assert!(found.is_empty(), "unsafe outside tools and apps: {found:?}");
}

/// The lints that hold the rules (S4): removing one, or loosening it, fails here first.
const HELD: [&str; 12] = [
    "unwrap_used",
    "expect_used",
    "panic",
    "unreachable",
    "string_slice",
    "print_stdout",
    "print_stderr",
    "exit",
    "disallowed_types",
    "too_many_lines",
    "allow_attributes_without_reason",
    "unreachable_pub",
];

/// S4: the workspace's lints are hard: each is denied, and none of those that hold a rule is gone.
#[test]
fn the_workspace_lints_are_denied_and_kept() {
    let manifest = std::fs::read_to_string(root().join("Cargo.toml")).unwrap();
    let mut section = "";
    let mut levels = vec![];
    for line in manifest.lines() {
        if line.starts_with('[') {
            section = line;
        } else if section.starts_with("[workspace.lints")
            && let Some((name, level)) = line.split_once('=')
        {
            levels.push((name.trim().to_string(), level.trim().to_string()));
        }
    }
    let loose: Vec<_> = (levels.iter())
        .filter(|(_, level)| !matches!(level.as_str(), "\"deny\"" | "\"forbid\""))
        .collect();
    assert!(
        loose.is_empty(),
        "a workspace lint is not denied: {loose:?}"
    );
    let gone: Vec<_> = (HELD.iter())
        .filter(|held| !levels.iter().any(|(name, _)| name == *held))
        .collect();
    assert!(gone.is_empty(), "lints that hold a rule are gone: {gone:?}");
}

/// S4: working code is never exempt as a whole: an `allow` sits on the item it is about. Build
/// scripts fail the build by panicking, and say so once.
#[test]
fn no_working_file_turns_a_lint_off_whole() {
    let mut found = vec![];
    for dir in crates() {
        let mut files = vec![];
        let working = |path: &Path| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.ends_with(".rs")
                && !name.ends_with("_tests.rs")
                && !matches!(name, "testing.rs" | "test_support.rs" | "fakes.rs")
        };
        walk(&dir.join("src"), &working, &mut files);
        for path in files {
            let text = std::fs::read_to_string(&path).unwrap();
            let whole = |line: &str| line.starts_with("#![allow") || line.starts_with("#![expect");
            if text.lines().any(|line| whole(line.trim_start())) {
                found.push(path.strip_prefix(root()).unwrap().display().to_string());
            }
        }
    }
    assert!(
        found.is_empty(),
        "a whole file is exempt from the lints: {found:?}"
    );
}
