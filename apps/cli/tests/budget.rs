//! The size budget: the whole project's working code stays under a hard limit. A small agent is a
//! goal, not an accident; when a change does not fit, code is removed, the limit is not raised.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Lines of working code in the whole workspace, at most. Hard: not to be raised.
const LIMIT: usize = 17_000;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Test code does not count: sibling `*_tests.rs` files and the test-only helpers.
fn counted(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let source = name.ends_with(".rs") || name.ends_with(".js");
    source && !name.ends_with("_tests.rs") && !matches!(name, "testing.rs" | "fakes.rs")
}

/// Lines that are code: not blank and not a comment. JavaScript block comments are skipped too
/// (a Rust line may start with `*` as a dereference, so Rust skips only `//` lines).
fn code_lines(path: &Path) -> usize {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let javascript = path.extension().is_some_and(|ext| ext == "js");
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .filter(|line| !(javascript && (line.starts_with("/*") || line.starts_with('*'))))
        .count()
}

fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, found);
        } else if counted(&path) {
            found.push(path);
        }
    }
}

/// Each package's `src/` (its `tests/` directory is test code and does not count).
fn measure() -> (BTreeMap<String, usize>, Vec<(usize, PathBuf)>) {
    let mut packages = BTreeMap::new();
    let mut files = vec![];
    for group in ["packages", "apps"] {
        for entry in std::fs::read_dir(root().join(group)).unwrap().flatten() {
            let mut found = vec![];
            sources(&entry.path().join("src"), &mut found);
            let name = format!("{group}/{}", entry.file_name().to_string_lossy());
            for path in found {
                let lines = code_lines(&path);
                *packages.entry(name.clone()).or_default() += lines;
                files.push((lines, path));
            }
        }
    }
    files.sort_by_key(|file| std::cmp::Reverse(file.0));
    (packages, files)
}

#[test]
fn the_project_stays_under_its_size_limit() {
    let (packages, files) = measure();
    let total: usize = packages.values().sum();
    let mut report = format!("{total} lines of working code, the hard limit is {LIMIT}\n");
    for (name, lines) in &packages {
        report.push_str(&format!("  {lines:>6}  {name}\n"));
    }
    report.push_str("the largest files:\n");
    for (lines, path) in files.iter().take(10) {
        let shown = path.strip_prefix(root()).unwrap_or(path);
        report.push_str(&format!("  {lines:>6}  {}\n", shown.display()));
    }
    println!("{report}");
    assert!(
        total <= LIMIT,
        "{report}remove code to make room; the limit is not raised"
    );
}

#[test]
fn comments_blanks_and_tests_do_not_count() {
    let dir = std::env::temp_dir().join(format!("llx-budget-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rust = dir.join("x.rs");
    std::fs::write(
        &rust,
        "//! doc\n\nfn main() {\n    // why\n    *x = 1;\n}\n",
    )
    .unwrap();
    let script = dir.join("x.js");
    std::fs::write(&script, "/* a\n * block\n */\nconst a = 1;\n").unwrap();
    assert_eq!(
        code_lines(&rust),
        3,
        "the dereference counts, the comments do not"
    );
    assert_eq!(code_lines(&script), 1);
    assert!(!counted(Path::new("agent_tests.rs")));
    assert!(!counted(Path::new("testing.rs")));
    assert!(counted(Path::new("agent.rs")));
    std::fs::remove_dir_all(&dir).unwrap();
}
