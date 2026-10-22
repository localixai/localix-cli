use super::url_in;

#[test]
fn finds_the_site_a_goal_names() {
    assert_eq!(
        url_in("open https://example.com/a?b=1 now"),
        Some("https://example.com/a?b=1".into())
    );
    assert_eq!(
        url_in("finde einen Artikel auf wikipedia.org über Gödel"),
        Some("https://wikipedia.org".into())
    );
    assert_eq!(url_in("write to me@mail.com"), None);
    assert_eq!(url_in("version 1.2 is out."), None);
    assert_eq!(url_in("Search flights"), None);
}
