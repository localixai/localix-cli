//! The words of the trace that are not blocks of their own: a model turn for `-v`, what the run
//! spent, and the answer split between stdout and stderr.

use contracts::chat::Usage;

use super::Summary;

/// One model turn, for `-v`.
pub fn turn(usage: &Usage) -> String {
    format!(
        "  · {} · {} in · {} out · ${:.4} · {:.1}s",
        usage.model,
        thousands(usage.input_tokens),
        thousands(usage.output_tokens),
        usage.cost,
        usage.millis as f64 / 1000.0
    )
}

/// Under the closing rule: each model's calls, tokens, money and time, and the session.
pub fn spend(summary: &Summary) -> String {
    let (agent, decision) = (summary.agent, summary.decision);
    let seconds = |ms: u64| ms as f64 / 1000.0;
    let mut out = String::new();
    if agent.calls > 0 {
        out.push_str(&format!(
            "agent    {} · {} · {} out · ${:.4} · {:.1}s\n",
            agent.model,
            tokens_in(agent.input_tokens, agent.cached_tokens),
            thousands(agent.output_tokens),
            agent.cost,
            seconds(agent.millis)
        ));
    }
    if decision.calls > 0 {
        out.push_str(&format!(
            "hands    {} · {} calls · {} in · {} out · ${:.4} · {:.1}s\n",
            decision.model,
            decision.calls,
            thousands(decision.input_tokens),
            thousands(decision.output_tokens),
            decision.cost,
            seconds(decision.millis)
        ));
    }
    if !summary.session.is_empty() {
        out.push_str(&format!("session  {}\n", summary.session));
    }
    out
}

/// What goes to stdout and what to stderr at the end: the answer alone on stdout, or with
/// `--schema` the data alone on stdout (one line, for a pipeline) and the prose on stderr.
pub fn answer(summary: &Summary) -> (String, String) {
    let prose = summary.answer.trim();
    let line = |text: &str| match text.is_empty() {
        true => String::new(),
        false => format!("{text}\n"),
    };
    match summary.data {
        Some(Ok(data)) => (data.to_string(), line(prose)),
        Some(Err(why)) => (String::new(), format!("{}llx: {why}\n", line(prose))),
        None => (prose.to_string(), String::new()),
    }
}

/// `12,345 in (2,000 cached)`, or `12,345 in` when nothing was cached: `/usage` says it so too.
pub fn tokens_in(input: u64, cached: u64) -> String {
    match cached {
        0 => format!("{} in", thousands(input)),
        cached => format!("{} in ({} cached)", thousands(input), thousands(cached)),
    }
}

/// `12,345`
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
#[path = "human_tests.rs"]
mod tests;
