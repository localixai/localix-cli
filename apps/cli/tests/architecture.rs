//! The layering rules (AGENTS.md, L1-L5), checked on the manifests and sources of the workspace.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every workspace package: (directory group, package name) → the workspace packages it depends on.
fn packages() -> BTreeMap<(String, String), Vec<String>> {
    let mut manifests = BTreeMap::new();
    for group in ["packages", "apps"] {
        for entry in std::fs::read_dir(root().join(group)).unwrap().flatten() {
            let Ok(text) = std::fs::read_to_string(entry.path().join("Cargo.toml")) else {
                continue;
            };
            let name = text
                .lines()
                .find_map(|line| line.strip_prefix("name = "))
                .unwrap()
                .trim_matches('"')
                .to_string();
            manifests.insert((group.to_string(), name), text);
        }
    }
    let names: Vec<String> = manifests.keys().map(|(_, name)| name.clone()).collect();
    manifests
        .into_iter()
        .map(|(key, text)| {
            let mut section = String::new();
            let mut uses = vec![];
            for line in text.lines() {
                if line.starts_with('[') {
                    section = line.to_string();
                } else if section.ends_with("dependencies]") {
                    let dependency = line.split(['.', ' ', '=']).next().unwrap_or("");
                    if names.iter().any(|name| name == dependency) {
                        uses.push(dependency.to_string());
                    }
                }
            }
            (key, uses)
        })
        .collect()
}

/// The packages that speak only in contracts: the agent's loop and the hands' loop.
const CORES: [&str; 2] = ["agent", "decision-loop"];

#[test]
fn l1_the_contracts_depend_on_nothing_in_the_workspace() {
    let packages = packages();
    let contracts = &packages[&("packages".into(), "contracts".into())];
    assert!(contracts.is_empty(), "contracts depends on {contracts:?}");
}

#[test]
fn l2_the_agent_and_the_hands_know_only_the_contracts() {
    let packages = packages();
    for core in CORES {
        let uses = &packages[&("packages".into(), core.into())];
        assert_eq!(uses, &["contracts"], "{core} depends on {uses:?}");
    }
}

#[test]
fn l3_tools_providers_and_stores_know_only_the_contracts() {
    for ((group, name), uses) in packages() {
        if group == "packages" && name != "contracts" && !CORES.contains(&name.as_str()) {
            assert_eq!(uses, ["contracts"], "{name} depends on {uses:?}");
        }
    }
}

#[test]
fn l4_only_apps_compose_packages() {
    let packages = packages();
    let apps: Vec<&String> = packages
        .keys()
        .filter(|(group, _)| group == "apps")
        .map(|(_, name)| name)
        .collect();
    for ((group, name), uses) in &packages {
        for used in uses {
            assert!(
                !apps.contains(&used),
                "{group}/{name} depends on the app {used}"
            );
        }
    }
    assert!(packages.contains_key(&("apps".into(), "localix-cli".into())));
}

/// Words that name a particular tool or provider: the contracts, the agent and the hands must not
/// say them.
const TOOL_WORDS: &[&str] = &[
    "browser",
    "webview",
    "web page",
    "page",
    "openrouter",
    "jev",
    "url",
    "html",
    "bash",
    "sh -c",
];

#[test]
fn l5_the_contracts_the_agent_and_the_hands_name_no_tool_or_provider() {
    for package in ["contracts", "agent", "decision-loop"] {
        let dir = root().join("packages").join(package).join("src");
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let text = std::fs::read_to_string(entry.path())
                .unwrap()
                .to_lowercase();
            for word in TOOL_WORDS {
                let named = text.match_indices(word).any(|(at, _)| {
                    !text[at + word.len()..].starts_with(|c: char| c.is_alphanumeric())
                        && !text[..at].ends_with(|c: char| c.is_alphanumeric())
                });
                assert!(
                    !named,
                    "{} names \"{word}\"",
                    entry.path().strip_prefix(root()).unwrap().display()
                );
            }
        }
    }
}

/// Every `.rs` file under the workspace's `src` directories.
fn sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    let mut out = vec![];
    for group in ["packages", "apps"] {
        for entry in std::fs::read_dir(root().join(group)).unwrap().flatten() {
            walk(&entry.path().join("src"), &mut out);
        }
    }
    out
}

/// Unit tests live beside the code they test, never inside it: `x.rs` ends with
/// `#[cfg(test)] #[path = "x_tests.rs"] mod tests;` and the tests are in `x_tests.rs`.
#[test]
fn tests_live_in_their_own_files() {
    let sources = sources();
    assert!(sources.len() > 20, "found only {} sources", sources.len());
    for path in &sources {
        let text = std::fs::read_to_string(path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let shown = path.strip_prefix(root()).unwrap().display();
        if let Some(stem) = name.strip_suffix("_tests.rs") {
            let code = path.with_file_name(format!("{stem}.rs"));
            let wired = std::fs::read_to_string(&code)
                .is_ok_and(|code| code.contains(&format!("#[path = \"{name}\"]")));
            assert!(wired, "{shown} is not wired from {stem}.rs");
            continue;
        }
        assert!(
            !text.contains("#[test]") && !text.contains("mod tests {"),
            "{shown} has tests inside: move them to its _tests.rs"
        );
    }
}
