//! A tool's unified edit diff as numbered, coloured rows, never clipped log output.

use ratatui::text::{Line, Span};

use super::super::{
    style,
    wrap::{hard, width as columns},
};
use crate::printable::printable;

pub(super) fn render(diff: &str, width: usize) -> Vec<Line<'static>> {
    let (mut old, mut new) = (0, 0);
    let mut out: Vec<Line<'static>> = vec![];
    for line in diff.lines() {
        if let Some(header) = line.strip_prefix("@@ -") {
            if !out.is_empty() {
                out.push(Line::from(Span::styled("    …", style::dim())));
            }
            let mut parts = header.split_whitespace();
            old = number(parts.next().unwrap_or_default());
            new = number(parts.next().unwrap_or_default().trim_start_matches('+'));
            continue;
        }
        let (before, after, mark, colour, text) = match line.as_bytes().first() {
            Some(b'-') => {
                let at = old;
                old += 1;
                (
                    at.to_string(),
                    String::new(),
                    '-',
                    style::failed(),
                    &line[1..],
                )
            }
            Some(b'+') => {
                let at = new;
                new += 1;
                (
                    String::new(),
                    at.to_string(),
                    '+',
                    style::done(),
                    &line[1..],
                )
            }
            Some(b' ') => {
                let at = (old, new);
                old += 1;
                new += 1;
                (
                    at.0.to_string(),
                    at.1.to_string(),
                    ' ',
                    style::dim(),
                    &line[1..],
                )
            }
            _ => {
                for text in hard(&printable(line), width.saturating_sub(4).max(1)) {
                    out.push(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(text, style::dim()),
                    ]));
                }
                continue;
            }
        };
        let gutter = format!("    {before:>3} {after:>3} {mark} ");
        let room = width.saturating_sub(columns(&gutter)).max(1);
        for (at, text) in hard(&printable(text), room).into_iter().enumerate() {
            let prefix = if at == 0 {
                gutter.clone()
            } else {
                " ".repeat(columns(&gutter))
            };
            let row = Line::from(vec![
                Span::styled(prefix, colour),
                Span::styled(text, colour),
            ]);
            // Extremely narrow terminals still retain the signs and line numbers.
            for mut row in super::super::wrap::fit(&row, width) {
                if mark != ' ' {
                    use ratatui::style::{Color, Style};
                    let background = if mark == '+' {
                        Color::Green
                    } else {
                        Color::Red
                    };
                    row.style = Style::new().bg(background).fg(Color::Black);
                    for span in &mut row.spans {
                        span.style = row.style;
                    }
                    // Explicit padding also carries the band into scrollback's ANSI output.
                    let padding = width.saturating_sub(super::super::wrap::line_width(&row));
                    row.spans.push(Span::raw(" ".repeat(padding)));
                }
                out.push(row);
            }
        }
    }
    out
}

fn number(range: &str) -> usize {
    range
        .split(',')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}
