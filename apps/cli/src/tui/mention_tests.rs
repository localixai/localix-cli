use super::*;

fn files() -> Files {
    Files::with(
        [
            "README.md",
            "src/main.rs",
            "src/tui/markdown.rs",
            "src/tui/mod.rs",
            "site/index.html",
        ]
        .map(str::to_string)
        .to_vec(),
    )
}

#[test]
fn a_file_is_found_by_its_name_then_its_path_then_its_letters() {
    assert_eq!(files().matching("mod"), ["src/tui/mod.rs"]);
    assert_eq!(
        files().matching("ma"),
        ["src/main.rs", "src/tui/markdown.rs"]
    );
    assert_eq!(
        files().matching("tui"),
        ["src/tui/mod.rs", "src/tui/markdown.rs"]
    );
    assert_eq!(files().matching("sih"), ["site/index.html"]);
    assert!(files().matching("zzz").is_empty());
}

#[test]
fn nothing_is_offered_before_the_list_is_read() {
    assert!(Files::new(None).matching("").is_empty());
}
