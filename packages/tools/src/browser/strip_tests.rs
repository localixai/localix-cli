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
            group: "llx".into(),
        },
        Item {
            key: Key::Term(2),
            title: "session 2 · python3".into(),
            group: "llx".into(),
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
        group: String::new(),
    }];
    let script = script(&items, Key::Web(1));
    assert!(script.contains("\"title\":\"New tab\""), "{script}");
}

fn item(key: Key, title: &str, group: &str) -> Item {
    Item {
        key,
        title: title.into(),
        group: group.into(),
    }
}

#[test]
fn one_project_draws_as_it_always_has() {
    let items = [
        item(Key::Web(1), "Docs", "llx"),
        item(Key::Term(2), "make", "llx"),
    ];
    assert_eq!(
        script(&items, Key::Web(1)),
        r#"S({"shown":"w1","tabs":[{"key":"w1","kind":"web","title":"Docs"},{"key":"t2","kind":"term","title":"make"}]})"#
    );
}

#[test]
fn several_projects_draw_their_tabs_together_each_saying_its_project() {
    let items = [
        item(Key::Web(1), "Docs", "llx"),
        item(Key::Term(2), "make", "<b>shop</b>"),
        item(Key::Web(3), "Issues", "llx"),
    ];
    let script = script(&items, Key::Web(1));
    let json = script
        .strip_prefix("S(")
        .and_then(|s| s.strip_suffix(')'))
        .unwrap();
    let data: serde_json::Value = serde_json::from_str(json).unwrap();
    let tabs = data["tabs"].as_array().unwrap();
    let keys: Vec<&str> = tabs
        .iter()
        .map(|tab| tab["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["w1", "w3", "t2"]);
    let labels: Vec<&serde_json::Value> = tabs.iter().map(|tab| &tab["group"]).collect();
    assert_eq!(
        labels,
        [
            &serde_json::json!("llx"),
            &serde_json::json!("llx"),
            &serde_json::json!("<b>shop</b>")
        ]
    );
    // A project's name is set as text too.
    assert!(PAGE.contains("pill.textContent = name"));
}

#[test]
fn the_toolbar_and_the_history_button_say_what_they_are() {
    assert_eq!(click("back"), Some(Click::Back));
    assert_eq!(click("forward"), Some(Click::Forward));
    assert_eq!(click("history"), Some(Click::History));
    assert_eq!(click("visit 2"), Some(Click::Visit(2)));
    for junk in [
        "back w1",
        "visit",
        "visit x",
        "visit -1",
        "visit 1 2",
        "History",
    ] {
        assert_eq!(click(junk), None, "{junk:?}");
    }
}

#[test]
fn the_history_menu_reaches_the_page_as_data() {
    use super::super::history::Visit;
    let visit = Visit {
        url: "https://a.example/\"</script>".into(),
        title: "A".into(),
    };
    let open = menu(Some(std::slice::from_ref(&visit)));
    assert!(open.starts_with("M([{"), "{open}");
    assert!(open.contains(r#""title":"A""#), "{open}");
    assert!(open.contains(r#"\"</script>"#), "{open}");
    assert_eq!(menu(None), "M(null)");
}
