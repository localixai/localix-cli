//! How a caller's script is started in the page.

/// `press` and `sleep`, handed to every script.
const HELPERS: &str = include_str!("probe/helpers.js");
/// How long a script may run before its helpers stop it: a little under the driver's wait for it.
const LIMIT: std::time::Duration = std::time::Duration::from_millis(19_500);
/// Where the last statement may start, tried at most: each try that does not parse costs a call.
const MAX_CUTS: usize = 6;

/// The ways to start a caller's script, tried in order until one parses (a parse error runs
/// nothing, so nothing runs twice): an expression; statements whose last line is returned; a
/// function body with its own `return`. Each hands its promise to the probe, which answers over
/// IPC. Run natively, not through `eval`, so a page's CSP cannot refuse it. The helpers come in as
/// an outer function's parameters, so a script may declare the same names itself.
pub(super) fn script_forms(source: &str) -> Vec<String> {
    let helpers = HELPERS.replace("__SCRIPT_MS__", &LIMIT.as_millis().to_string());
    let start = |body: &str| {
        format!(
            "window.__dloop_call ? window.__dloop_call(__ID__, \"settled\", (({{ press, sleep }}) => (async () => {{\n{body}\n}})())({helpers})) : false"
        )
    };
    let source = source.trim().trim_end_matches(';');
    let mut forms = vec![start(&format!("return ({source}\n)"))];
    // The last statement starts after a `;`, a line break or a block (`while (…) { … } total`):
    // the last few of each are tried, latest first, since an object literal has braces too.
    let mut cuts: Vec<usize> = source
        .match_indices([';', '\n', '}'])
        .map(|(at, _)| at + 1)
        .rev()
        .take(MAX_CUTS)
        .collect();
    cuts.sort_unstable_by(|a, b| b.cmp(a));
    cuts.dedup();
    for at in cuts.into_iter().filter(|at| {
        !source[*at..].trim().is_empty() && !source[*at..].trim_start().starts_with("return")
    }) {
        forms.push(start(&format!(
            "{}\nreturn ({}\n)",
            &source[..at],
            source[at..].trim()
        )));
    }
    forms.push(start(source));
    forms
}

#[cfg(test)]
#[path = "script_tests.rs"]
mod tests;
