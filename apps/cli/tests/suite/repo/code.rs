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
const SAFE: [&str; 6] = [
    "packages/contracts",
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
