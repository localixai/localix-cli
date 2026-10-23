use super::*;

#[test]
fn a_click_names_a_tab_and_what_to_do_with_it() {
    assert_eq!(click("show w3"), Some(Click::Show(Key::Web(3))));
    assert_eq!(click("show t1"), Some(Click::Show(Key::Term(1))));
    assert_eq!(click("close t2"), Some(Click::Close(Key::Term(2))));
    assert_eq!(click("close w1"), Some(Click::Close(Key::Web(1))));
}

#[test]
fn anything_else_from_the_strip_is_ignored() {
    for junk in [
        "",
        "show",
        "show x3",
        "show w",
        "show w-1",
        "open w1",
        "show w1 t2",
        "show w99999999999",
    ] {
        assert_eq!(click(junk), None, "{junk:?}");
    }
}

#[test]
fn the_tabs_reach_the_page_as_data() {
    let items = [
        Item {
            key: Key::Web(1),
            title: "</script><img src=x onerror=alert(1)>".into(),
        },
        Item {
            key: Key::Term(2),
            title: "session 2 · python3".into(),
        },
    ];
    let script = script(&items, Key::Term(2));
    assert!(
        script.starts_with("S(") && script.ends_with(')'),
        "{script}"
    );
    let json = script
        .strip_prefix("S(")
        .and_then(|s| s.strip_suffix(')'))
        .unwrap();
    let data: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(data["shown"], "t2");
    assert_eq!(data["tabs"][0]["key"], "w1");
    assert_eq!(data["tabs"][0]["kind"], "web");
    assert_eq!(
        data["tabs"][0]["title"],
        "</script><img src=x onerror=alert(1)>"
    );
    assert_eq!(data["tabs"][1]["kind"], "term");
    // The page sets titles as text, never as markup.
    assert!(PAGE.contains("textContent") && !PAGE.contains("innerHTML"));
}

#[test]
fn an_empty_title_reads_as_what_the_tab_is() {
    let items = [Item {
        key: Key::Web(1),
        title: String::new(),
    }];
    let script = script(&items, Key::Web(1));
    assert!(script.contains("\"title\":\"New tab\""), "{script}");
}
