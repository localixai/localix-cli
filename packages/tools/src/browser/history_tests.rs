use super::*;

#[test]
fn the_newest_page_is_first_and_a_page_is_listed_once() {
    let history = History::default();
    history.visited("https://a.example/");
    history.visited("https://b.example/");
    history.titled("https://a.example/", "  A  ");
    history.visited("https://a.example/");
    let urls: Vec<_> = history
        .list()
        .into_iter()
        .map(|v| (v.url, v.title))
        .collect();
    assert_eq!(
        urls,
        [
            ("https://a.example/".to_string(), "A".to_string()),
            ("https://b.example/".to_string(), String::new()),
        ]
    );
}

#[test]
fn only_web_pages_are_kept_and_only_so_many() {
    let history = History::default();
    history.visited("about:blank");
    history.visited("data:text/html,x");
    assert!(history.list().is_empty());
    for n in 0..KEEP + 5 {
        history.visited(&format!("https://site.example/{n}"));
    }
    let list = history.list();
    assert_eq!(list.len(), KEEP);
    assert_eq!(list[0].url, format!("https://site.example/{}", KEEP + 4));
}
