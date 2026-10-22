//! How the agent's script is started in the page.

/// The ways to start the agent's script, tried in order until one parses (a parse error runs
/// nothing, so nothing runs twice): an expression; statements whose last line is returned; a
/// function body with its own `return`. Each hands its promise to the probe, which answers over
/// IPC. Run natively, not through `eval`, so a page's CSP cannot refuse it.
pub(super) fn script_forms(source: &str) -> Vec<String> {
    let start = |body: &str| {
        format!(
            "window.__lxsurf_call ? window.__lxsurf_call(__ID__, \"settled\", (async () => {{\n{body}\n}})()) : false"
        )
    };
    let source = source.trim().trim_end_matches(';');
    let mut forms = vec![start(&format!("return ({source}\n)"))];
    let cut = source.rfind([';', '\n']).map(|at| at + 1);
    if let Some(at) = cut.filter(|at| {
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
