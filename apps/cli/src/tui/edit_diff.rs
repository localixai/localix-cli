//! A tool's unified edit diff as numbered, coloured rows, never clipped log output.

use ratatui::text::{Line, Span};

use super::super::{
    style,
    wrap::{hard, width as columns},
};
use crate::tui::printable::printable;

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
                    line.get(1..).unwrap_or_default(),
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
                    line.get(1..).unwrap_or_default(),
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
                    line.get(1..).unwrap_or_default(),
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
        // Only the sign and the code carry the colour: the numbers stay dim, nothing is banded.
        let numbers = format!("    {before:>3} {after:>3} ");
        let sign = format!("{mark} ");
        let indent = columns(&numbers) + columns(&sign);
        let room = width.saturating_sub(indent).max(1);
        for (at, text) in hard(&printable(text), room).into_iter().enumerate() {
            let row = if at == 0 {
                Line::from(vec![
                    Span::styled(numbers.clone(), style::dim()),
                    Span::styled(sign.clone(), colour),
                    Span::styled(text, colour),
                ])
            } else {
                Line::from(vec![
                    Span::raw(" ".repeat(indent)),
                    Span::styled(text, colour),
                ])
            };
            // Extremely narrow terminals still retain the signs and line numbers.
            out.extend(super::super::wrap::fit(&row, width));
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
