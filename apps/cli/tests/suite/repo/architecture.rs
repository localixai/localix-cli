//! The layering rules (ARCHITECTURE.md, L1-L8), checked on the manifests and sources of the workspace.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{root, walk};

/// The directories packages live in: the model providers have one of their own.
const GROUPS: [&str; 3] = ["packages", "packages/providers", "apps"];

/// Every workspace package: (directory group, package name) → the workspace packages it depends on.
fn packages() -> BTreeMap<(String, String), Vec<String>> {
    let mut manifests = BTreeMap::new();
    for group in GROUPS {
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

/// What the model providers share: plumbing with no port, which knows no workspace package.
const KIT: &str = "provider-kit";
/// Primitives the app composes (the lanes' queue): no port, no workspace package.
const PRIMITIVES: [&str; 1] = ["queue"];

#[test]
fn l3_tools_providers_and_stores_know_only_the_contracts() {
    let packages = packages();
    for ((group, name), uses) in &packages {
        if group == "packages" && PRIMITIVES.contains(&name.as_str()) {
            assert!(uses.is_empty(), "{name} depends on {uses:?}");
        } else if group == "packages"
            && !matches!(name.as_str(), "contracts" | WIRE | CLIENT)
            && !CORES.contains(&name.as_str())
        {
            assert_eq!(uses, &["contracts"], "{name} depends on {uses:?}");
        }
    }
    let providers: Vec<_> = packages
        .iter()
        .filter(|((group, _), _)| group == "packages/providers")
        .collect();
    assert!(
        providers.len() >= 3,
        "the kit and two providers: {providers:?}"
    );
    for ((_, name), uses) in providers {
        match name.as_str() {
            KIT => assert!(uses.is_empty(), "the kit depends on {uses:?}"),
            _ => assert_eq!(uses, &["contracts", KIT], "{name} depends on {uses:?}"),
        }
    }
}

/// What the interface and the server say to each other.
const WIRE: &str = "protocol";

/// L6: the wire is a leaf: no workspace package, nothing but serde to say it with.
#[test]
fn l6_the_protocol_is_a_leaf() {
    let packages = packages();
    let uses = &packages[&("packages".into(), WIRE.into())];
    assert!(uses.is_empty(), "the protocol depends on {uses:?}");
    let manifest = std::fs::read_to_string(root().join("packages/protocol/Cargo.toml")).unwrap();
    let mut section = "";
    let mut external = vec![];
    for line in manifest.lines() {
        if line.starts_with('[') {
            section = line;
        } else if section == "[dependencies]"
            && let Some((name, _)) = line.split_once('=')
        {
            external.push(name.trim().to_string());
        }
    }
    assert_eq!(
        external,
        ["serde", "serde_json"],
        "the protocol uses {external:?}"
    );
}

/// The interface's side of the seam.
const CLIENT: &str = "client";

/// L8: the client knows the wire and nothing else: no session, tool, model or store.
#[test]
fn l8_the_client_knows_only_the_protocol() {
    let packages = packages();
    let uses = &packages[&("packages".into(), CLIENT.into())];
    assert_eq!(uses, &[WIRE], "the client depends on {uses:?}");
}

/// What the interface must not name (L7): the app's other modules, and every workspace package
/// but the protocol and the client.
const BEYOND_THE_INTERFACE: &[&str] = &[
    "crate::",
    "contracts::",
    "tools::",
    "agent::",
    "decision_loop::",
    "session::",
    "queue::",
    "provider_kit::",
    "models::",
    "openai::",
    "openrouter::",
];

/// The interfaces (L7): the terminal's, and the tool clients' (`llx mcp`, W9).
const INTERFACES: [&str; 2] = ["tui", "mcp"];

/// L7: each interface (`apps/cli/src/tui`, `apps/cli/src/mcp`, their tests too) speaks only
/// through the protocol and the client: what it shows and what it may do, the server says.
#[test]
fn l7_the_interface_knows_only_the_protocol_and_the_client() {
    for interface in INTERFACES {
        let files = sources_in(&root().join("apps/cli/src").join(interface));
        assert!(!files.is_empty(), "no sources of the interface {interface}");
        for path in files
            .iter()
            .filter(|p| p.extension().is_some_and(|e| e == "rs"))
        {
            let text = std::fs::read_to_string(path).unwrap();
            let code = text.lines().filter(|l| !l.trim_start().starts_with("//"));
            for line in code {
                let named = named_beyond(line, interface);
                assert!(
                    named.is_none(),
                    "{} names `{}` in `{}`: the interface knows only `crate::{interface}`, \
                     `protocol` and `client` (L7); have the server say it through the protocol \
                     (an `Event`, a `Done`), or move the presentation into `{interface}`",
                    path.strip_prefix(root()).unwrap().display(),
                    named.unwrap_or_default(),
                    line.trim()
                );
            }
        }
    }
}

/// The first path in `line` that leaves `interface`: one of [`BEYOND_THE_INTERFACE`] at the
/// start of a path (not a module of the interface's own, `super::queue::`), but its own
/// `crate::<interface>`.
fn named_beyond(line: &str, interface: &str) -> Option<&'static str> {
    let own = format!("crate::{interface}");
    BEYOND_THE_INTERFACE.iter().copied().find(|word| {
        line.match_indices(word).any(|(at, _)| {
            let before = &line[..at];
            let segment = |s: &str| s.ends_with(|c: char| c.is_alphanumeric() || c == '_');
            let inner = segment(before) || before.strip_suffix("::").is_some_and(segment);
            let own = *word == "crate::" && line[at..].starts_with(&own);
            !(inner || own)
        })
    })
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
    assert!(packages.contains_key(&("apps".into(), "llx".into())));
}

/// Words that name a particular tool or provider: the contracts, the agent and the hands must not
/// say them.
const TOOL_WORDS: &[&str] = &[
    "browser",
    "webview",
    "web page",
    "page",
    "openrouter",
    "openai",
    "chatgpt",
    "jev",
    "url",
    "html",
    "bash",
    "sh -c",
    // Model vendors and families, and the catalog. Not "claude": the prompt names CLAUDE.md.
    "anthropic",
    "gpt",
    "gemini",
    "deepseek",
    "models.dev",
];

#[test]
fn l5_the_contracts_the_agent_and_the_hands_name_no_tool_or_provider() {
    for package in ["contracts", "agent", "decision-loop"] {
        let dir = root().join("packages").join(package).join("src");
        let files = sources_in(&dir);
        assert!(!files.is_empty(), "no sources in {}", dir.display());
        for path in files {
            let text = std::fs::read_to_string(&path).unwrap().to_lowercase();
            for word in TOOL_WORDS {
                let named = text.match_indices(word).any(|(at, _)| {
                    !text[at + word.len()..].starts_with(|c: char| c.is_alphanumeric())
                        && !text[..at].ends_with(|c: char| c.is_alphanumeric())
                });
                assert!(
                    !named,
                    "{} names \"{word}\"",
                    path.strip_prefix(root()).unwrap().display()
                );
            }
        }
    }
}

/// Every file under `dir` (code, the prompt), at any depth.
fn sources_in(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    walk(dir, &|_| true, &mut out);
    out
}

/// Every `.rs` file under the workspace's `src` directories.
fn sources() -> Vec<PathBuf> {
    let mut out = vec![];
    for group in GROUPS {
        for entry in std::fs::read_dir(root().join(group)).unwrap().flatten() {
            let rust = sources_in(&entry.path().join("src"))
                .into_iter()
                .filter(|path| path.extension().is_some_and(|ext| ext == "rs"));
            out.extend(rust);
        }
    }
    out
}

/// The files of the repository: those git tracks (files a run left in the tree are not the
/// repository's until they are added), else, with no git or no checkout (a source archive),
/// every file but the build's and git's own.
#[allow(
    clippy::disallowed_methods,
    reason = "a test's fixture: `git ls-files` ends by itself"
)]
fn repository_files() -> Vec<PathBuf> {
    let listed = std::process::Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root())
        .output();
    if let Ok(listed) = listed
        && listed.status.success()
        && !listed.stdout.is_empty()
    {
        return String::from_utf8_lossy(&listed.stdout)
            .split('\0')
            .filter(|name| !name.is_empty())
            .map(|name| root().join(name))
            .collect();
    }
    let mut out = vec![];
    for entry in std::fs::read_dir(root()).unwrap().flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if matches!(name.to_str(), Some("target" | ".git")) {
            continue;
        }
        match path.is_dir() {
            true => {
                let built = |path: &Path| path.components().any(|c| c.as_os_str() == "target");
                walk(&path, &|path| !built(path), &mut out);
            }
            false => out.push(path),
        }
    }
    out
}

/// The repository is English only: no Cyrillic in any text file of it, tests included.
#[test]
fn no_cyrillic_anywhere() {
    let files = repository_files();
    assert!(files.len() > 20, "found only {} files", files.len());
    let mut found = vec![];
    for path in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue; // gone, or not text (fonts, images)
        };
        if text.chars().any(|c| ('\u{400}'..='\u{52f}').contains(&c)) {
            found.push(
                path.strip_prefix(root())
                    .unwrap_or(&path)
                    .display()
                    .to_string(),
            );
        }
    }
    assert!(found.is_empty(), "Cyrillic in {found:?}");
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
