//! Server-sent events, read from a whole streamed body: each event's `data`, parsed as JSON.

use serde_json::Value;

/// The JSON `data` of each event in `body`, in order. Multi-line data is joined, comments and
/// `[DONE]` are skipped, and data that is not JSON is left out.
pub fn events(body: &str) -> Vec<Value> {
    let mut events = vec![];
    let mut data = String::new();
    let mut flush = |data: &mut String| {
        // `[DONE]` and anything else that is not JSON fail to parse and are left out.
        if let Ok(value) = serde_json::from_str(data) {
            events.push(value);
        }
        data.clear();
    };
    for line in body.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            flush(&mut data);
        } else if let Some(value) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(value.strip_prefix(' ').unwrap_or(value));
        }
    }
    flush(&mut data);
    events
}

#[cfg(test)]
#[path = "sse_tests.rs"]
mod tests;
