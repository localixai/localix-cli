//! Where the browser opens for a goal: the site it names, else a search page.

/// Where a goal that names no site starts.
pub const SEARCH_PAGE: &str = "https://duckduckgo.com/";

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

#[cfg(test)]
#[path = "start_tests.rs"]
mod tests;
