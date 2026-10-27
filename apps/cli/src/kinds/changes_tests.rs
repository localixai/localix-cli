use super::*;

const HEAD: &str = "### Page changes since you last read it whole\n";

fn page(lines: &[&str]) -> String {
    lines.join("\n")
}

#[test]
fn a_page_that_moved_a_little_reads_as_its_changes_after_the_line_above() {
    let before = page(&[
        "Shop — https://x/",
        "- heading \"Cart\"",
        "- text \"0 items\"",
        "- button \"Buy\"",
    ]);
    let after = page(&[
        "Shop — https://x/",
        "- heading \"Cart\"",
        "- text \"1 item\"",
        "- button \"Buy\"",
    ]);
    assert_eq!(
        changes(&before, &after).as_deref(),
        Some(&*format!(
            "{HEAD}  - heading \"Cart\"\n- - text \"0 items\"\n+ - text \"1 item\""
        ))
    );
    assert_eq!(
        changes(&after, &after).as_deref(),
        Some(&*format!("{HEAD}(none: it is as you read it)"))
    );
}

#[test]
fn of_several_equal_lines_the_one_that_changed_is_told_by_its_place() {
    // A game's stats: every value starts at 0, and only the score moved.
    let before = page(&[
        "- text \"Score\"",
        "- text \"0\"",
        "- text \"Moves\"",
        "- text \"0\"",
    ]);
    let after = page(&[
        "- text \"Score\"",
        "- text \"16\"",
        "- text \"Moves\"",
        "- text \"0\"",
    ]);
    assert_eq!(
        changes(&before, &after).as_deref(),
        Some(&*format!(
            "{HEAD}  - text \"Score\"\n- - text \"0\"\n+ - text \"16\""
        ))
    );
}

#[test]
fn a_line_gone_that_another_equal_one_still_shows_is_seen_gone() {
    let items = [
        "Bread", "Eggs", "Milk", "Milk", "Tea", "Apples", "Rice", "Soap",
    ];
    let list = |items: &[&str]| {
        let lines = items.iter().map(|item| format!("  - listitem \"{item}\""));
        std::iter::once("- list".to_string())
            .chain(lines)
            .chain(std::iter::once("- button \"Add\"".to_string()))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let (before, after) = (list(&items), list(&[&items[..3], &items[4..]].concat()));
    let told = changes(&before, &after).unwrap_or_default();
    assert!(told.contains("-   - listitem \"Milk\""), "{told}");
    assert!(!told.contains("(none"), "{told}");
}

#[test]
fn changes_apart_are_told_apart_with_a_mark_between() {
    let mut before: Vec<String> = (0..12).map(|n| format!("- text \"row {n}\"")).collect();
    let mut after = before.clone();
    after[2] = "- text \"row two\"".into();
    after[9] = "- text \"row nine\"".into();
    before.push("- button \"More\"".into());
    after.push("- button \"More\"".into());
    let told = changes(&before.join("\n"), &after.join("\n")).unwrap_or_default();
    let want = format!(
        "{HEAD}  - text \"row 1\"\n- - text \"row 2\"\n+ - text \"row two\"\n  …\n  - text \"row 8\"\n- - text \"row 9\"\n+ - text \"row nine\""
    );
    assert_eq!(told, want);
}

#[test]
fn many_changes_or_a_new_page_are_read_whole() {
    let before = page(&[
        "Shop — https://x/",
        "- heading \"Cart\"",
        "- text \"0 items\"",
    ]);
    let other = (0..60).fold(String::new(), |other, n| {
        other + &format!("- link \"{n}\"\n")
    });
    assert_eq!(changes(&before, &other), None);
    // Few lines, but told as changes they would be longer than the page itself.
    let long: Vec<String> = (0..20)
        .map(|n| format!("- text \"paragraph {n} of the old page\""))
        .collect();
    let short = page(&["Other — https://x/#b", "- heading \"Other\""]);
    assert_eq!(changes(&long.join("\n"), &short), None);
}

#[test]
fn a_call_that_changed_nothing_says_so_though_the_page_moved_since_it_was_read_whole() {
    let cart = |items: &str| {
        page(&[
            "Cart — https://x/",
            "- heading 1 \"Cart\"",
            "- listitem \"Lamp, $24\"",
            "  - button \"Add\"",
            "- listitem \"Mug, $9\"",
            "  - button \"Add\"",
            items,
        ])
    };
    let (read, added) = (cart("- text \"Items: 0\""), cart("- text \"Items: 2\""));
    let mut seen = Seen::read(&read);
    let first = seen.after(&added).unwrap_or_default();
    assert!(first.contains("+ - text \"Items: 2\""), "{first}");
    // A key the page did not take: nothing moved since the call before, which the page's changes
    // since its whole read would not tell.
    assert_eq!(seen.after(&added).as_deref(), Some(UNCHANGED));
    // Reloaded: back to what was read whole, a change from the call before.
    assert_eq!(
        seen.after(&read).as_deref(),
        Some(&*format!("{HEAD}(none: it is as you read it)"))
    );
    assert_eq!(seen.after(&read).as_deref(), Some(UNCHANGED));
}
