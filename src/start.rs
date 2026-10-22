//! What a run starts from when the command line does not say: the start page and whether the
//! window is visible.

use std::io::{IsTerminal, Read};

/// The goal from the command line, or from stdin for `-`.
pub fn read_goal(arg: Option<&str>) -> Result<String, String> {
    let goal = match arg {
        Some("-") => {
            let mut text = String::new();
            std::io::stdin()
                .read_to_string(&mut text)
                .map_err(|error| error.to_string())?;
            text
        }
        Some(goal) => goal.to_string(),
        None => return Err("no goal: lxsurf \"<what to do>\" [--url <page>]".into()),
    };
    let goal = goal.trim().to_string();
    if goal.is_empty() {
        return Err("the goal is empty".into());
    }
    Ok(goal)
}

/// The first web address in the goal: a URL, or a bare domain such as `wikipedia.org`.
pub fn url_in(goal: &str) -> Option<String> {
    goal.split_whitespace()
        .map(|word| word.trim_matches(|c: char| ",.;:!?()\"'«»".contains(c)))
        .find_map(|word| {
            if word.starts_with("http://") || word.starts_with("https://") {
                return Some(word.to_string());
            }
            let (host, _) = word.split_once('/').unwrap_or((word, ""));
            let tld = host.rsplit('.').next().unwrap_or("");
            let domain = host.contains('.')
                && !host.contains('@')
                && tld.len() >= 2
                && tld.chars().all(|c| c.is_ascii_alphabetic());
            domain.then(|| format!("https://{word}"))
        })
}

/// Invisible by default under CI or outside a terminal (a test runner, a pipe); `--show` and
/// `--headless` override it.
pub fn headless_by_default() -> bool {
    let ci = std::env::var_os("CI").is_some_and(|value| !value.is_empty() && value != "false");
    ci || !std::io::stderr().is_terminal()
}

#[cfg(target_os = "linux")]
pub fn has_display() -> bool {
    std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

#[cfg(not(target_os = "linux"))]
pub fn has_display() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::url_in;

    #[test]
    fn finds_the_site_a_goal_names() {
        assert_eq!(
            url_in("open https://example.com/a?b=1 now"),
            Some("https://example.com/a?b=1".into())
        );
        assert_eq!(
            url_in("найди статью на wikipedia.org про Гёделя"),
            Some("https://wikipedia.org".into())
        );
        assert_eq!(url_in("write to me@mail.com"), None);
        assert_eq!(url_in("version 1.2 is out."), None);
        assert_eq!(url_in("Search flights"), None);
    }
}
