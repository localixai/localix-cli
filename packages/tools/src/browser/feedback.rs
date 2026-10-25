//! Visual feedback: what the person commented on in a page (probe/feedback.js), put in words
//! for the agent. The capture comes from the page, so it is untrusted: every field is read as
//! text, bounded, and nothing in it is taken for an instruction (the window's tag says so).

use serde_json::Value;

use super::capture::Clip;

/// At most this many characters of any one field.
const FIELD: usize = 300;
/// At most this many characters of the person's note.
const NOTE: usize = 2000;

/// `value` as one line of at most `max` characters.
fn clip(value: Option<&Value>, max: usize) -> String {
    let text = match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        _ => return String::new(),
    };
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match line.chars().nth(max) {
        Some(_) => format!("{}…", line.chars().take(max).collect::<String>()),
        None => line,
    }
}

fn field(value: &Value, key: &str) -> String {
    clip(value.get(key), FIELD)
}

/// A box as `x,y w×h`.
fn boxed(value: &Value) -> String {
    let [x, y, width, height] = ["x", "y", "width", "height"].map(|key| field(value, key));
    format!("{x},{y} {width}×{height}")
}

/// Where the page says the element was written, when it says.
fn source(value: &Value) -> Option<String> {
    let file = field(value, "file");
    let components: Vec<String> = (value.get("components").and_then(Value::as_array))
        .map(|names| names.iter().map(|name| clip(Some(name), 60)).collect())
        .unwrap_or_default();
    let mut said = file;
    for key in ["line", "column"] {
        let at = field(value, key);
        if !said.is_empty() && !at.is_empty() {
            said = format!("{said}:{at}");
        }
    }
    if !components.is_empty() {
        let inside = format!("in {}", components.join(" < "));
        said = [said, inside].join(" ").trim().to_string();
    }
    (!said.is_empty()).then_some(said)
}

fn element(value: &Value, lines: &mut Vec<String>) {
    lines.push(format!("Element: {}", field(value, "tag")));
    for (label, key) in [("Role", "role"), ("Name", "name"), ("Text", "text")] {
        let said = field(value, key);
        if !said.is_empty() {
            lines.push(format!("{label}: {said}"));
        }
    }
    lines.push(format!("Selector: {}", field(value, "selector")));
    lines.push(format!("Path: {}", field(value, "path")));
    if let Some(at) = value.get("box") {
        lines.push(format!("Box: {}", boxed(at)));
    }
    if let Some(Value::Object(styles)) = value.get("styles") {
        let styles: Vec<String> = styles
            .iter()
            .take(12)
            .map(|(name, said)| {
                format!(
                    "{}: {}",
                    clip(Some(&Value::from(name.as_str())), 40),
                    clip(Some(said), 80)
                )
            })
            .collect();
        lines.push(format!("Styles: {}", styles.join("; ")));
    }
    if let Some(said) = value.get("source").and_then(source) {
        lines.push(format!("Source: {said}"));
    }
}

/// What the person said about a part of the page (`fb` from the probe), or nothing when it is
/// neither an element nor a region.
pub(super) fn said(fb: &Value) -> Option<String> {
    let note = clip(fb.get("note"), NOTE);
    let note = if note.is_empty() {
        "look at this"
    } else {
        note.as_str()
    };
    let mut lines = vec![format!(
        "The person commented (#{}) on {}: {note}",
        field(fb, "n"),
        field(fb, "url")
    )];
    if let Some(picked) = fb.get("element") {
        element(picked, &mut lines);
    } else if let Some(region) = fb.get("region") {
        lines.push(format!("Region: {}", boxed(region)));
        for member in fb
            .get("members")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .take(8)
        {
            let text = clip(member.get("text"), 80);
            lines.push(
                format!("- {} {text}", field(member, "selector"))
                    .trim_end()
                    .into(),
            );
        }
    } else {
        return None;
    }
    Some(lines.join("\n"))
}

/// Room around what was commented on in its picture, in CSS pixels: enough to see where it is.
const AROUND: f64 = 24.0;

/// The part of the view to picture for `fb`: its element's box or its region, with room around
/// it; none for a box of nothing.
pub(super) fn framed(fb: &Value) -> Option<Clip> {
    let at =
        (fb.get("element").and_then(|picked| picked.get("box"))).or_else(|| fb.get("region"))?;
    let number = |key: &str| at.get(key).and_then(Value::as_f64);
    let (x, y) = (number("x")?, number("y")?);
    let (width, height) = (number("width")?, number("height")?);
    if width < 1.0 || height < 1.0 {
        return None;
    }
    let (left, top) = ((x - AROUND).max(0.0), (y - AROUND).max(0.0));
    Some(Clip {
        x: left,
        y: top,
        width: x + width + AROUND - left,
        height: y + height + AROUND - top,
    })
}

#[cfg(test)]
#[path = "feedback_tests.rs"]
mod tests;
