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

#[test]
fn l1_the_contracts_depend_on_nothing_in_the_workspace() {
    let packages = packages();
    let agent = &packages[&("packages".into(), "agent".into())];
    assert!(agent.is_empty(), "agent depends on {agent:?}");
}

#[test]
fn l2_the_loop_knows_only_the_contracts() {
    let packages = packages();
    let uses = &packages[&("packages".into(), "decision-loop".into())];
    assert_eq!(uses, &["agent"], "decision-loop depends on {uses:?}");
}

#[test]
fn l3_tools_providers_and_stores_know_only_the_contracts() {
    for ((group, name), uses) in packages() {
        if group == "packages" && name != "agent" && name != "decision-loop" {
            assert_eq!(uses, ["agent"], "{name} depends on {uses:?}");
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
    assert!(packages.contains_key(&("apps".into(), "cli".into())));
}

/// Words that name a particular tool or provider: the loop and the contracts must not say them.
const TOOL_WORDS: &[&str] = &[
    "browser",
    "webview",
    "web page",
    "openrouter",
    "jev",
    "url",
    "html",
];

#[test]
fn l5_the_loop_and_the_contracts_name_no_tool_or_provider() {
    for package in ["agent", "decision-loop"] {
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
