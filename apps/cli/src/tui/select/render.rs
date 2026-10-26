//! A choice drawn: its title wrapped, a window of its options with the marked one, the line being
//! typed, and the hint for the keys.

use ratatui::text::{Line, Span};

use super::{Select, WINDOW};
use crate::tui::printable::printable;
use crate::tui::style;
use crate::tui::wrap::wrap;

impl Select {
    /// The rows at `width`, and the cursor's place among them while typing.
    pub(crate) fn render(&self, width: usize) -> (Vec<Line<'static>>, Option<(usize, usize)>) {
        let mut out: Vec<Line<'static>> = self
            .title
            .iter()
            .flat_map(|line| title_rows(line, width))
            .collect();
        if !out.is_empty() {
            out.push(Line::default());
        }
        let listed = if self.direct { vec![] } else { self.rows() };
        if let Some(search) = self.search.as_ref().filter(|_| !self.direct) {
            out.extend(search.render());
        }
        // A long list shows a window around the marked option, and how many are past it.
        let now = listed.iter().position(|at| *at == self.at).unwrap_or(0);
        let start = now
            .saturating_sub(WINDOW / 2)
            .min(listed.len().saturating_sub(WINDOW));
        let end = (start + WINDOW).min(listed.len());
        let more = |count: usize, arrow: &str| {
            Line::from(Span::styled(
                format!("  {arrow} {count} more"),
                style::dim(),
            ))
        };
        if start > 0 {
            out.push(more(start, "↑"));
        }
        for &at in &listed[start..end] {
            let choice = &self.choices[at];
            let marked = at == self.at;
            let mark = if marked {
                Span::styled("❯ ", style::bold())
            } else {
                Span::raw("  ")
            };
            let label_style = if marked {
                style::bold()
            } else {
                Default::default()
            };
            let check = match (self.multiple, self.checked.get(at)) {
                (true, Some(true)) => "[x] ",
                (true, Some(false)) if !choice.types => "[ ] ",
                _ => "",
            };
            let number = match self.search {
                Some(_) => String::new(),
                None => format!("{}. ", at + 1),
            };
            let mut line = Line::from(vec![
                Span::styled(format!("{number}{check}"), label_style),
                Span::styled(printable(&choice.label), label_style),
            ]);
            if !choice.detail.is_empty() {
                line.spans.push(Span::styled(
                    format!(" {}", printable(&choice.detail)),
                    style::dim(),
                ));
            }
            if let Some(adjustment) = self.adjustments.as_ref().and_then(|rows| rows.get(at))
                && let Some(value) = adjustment.values.get(adjustment.at)
            {
                line.spans.push(Span::styled(
                    format!("  ← {} →", printable(value)),
                    style::dim(),
                ));
            }
            out.extend(wrap(
                &line,
                width,
                &Line::from(mark),
                &Line::from(" ".repeat(number.len() + 2)),
            ));
        }
        if end < listed.len() {
            out.push(more(listed.len() - end, "↓"));
        }
        let mut cursor = None;
        if let Some(composer) = &self.typing {
            if !self.direct {
                out.push(Line::default());
            }
            let (rows, (row, column)) =
                composer.render(&Span::styled("  ❯ ", style::bold()), "Type here", width);
            cursor = Some((out.len() + row, column));
            out.extend(rows);
        }
        out.push(Line::default());
        out.push(Line::from(Span::styled(self.hint(), style::dim())));
        (out, cursor)
    }

    /// The keys' hint under the choice.
    fn hint(&self) -> String {
        match self.typing {
            Some(_) if self.direct => format!("  enter to send · {}", self.escape),
            Some(_) => "  enter to send · esc to go back".to_string(),
            None => {
                let (search, escape) = match &self.search {
                    Some(search) if !search.is_empty() => ("type to search · ", "esc to clear"),
                    Some(_) => ("type to search · ", self.escape.as_str()),
                    None => ("", self.escape.as_str()),
                };
                let check = if self.multiple {
                    "space to check · "
                } else {
                    ""
                };
                let adjust = if self.adjustments.is_some() {
                    "←→ reasoning · "
                } else {
                    ""
                };
                format!("  {search}↑↓ to choose · {adjust}{check}enter to confirm · {escape}")
            }
        }
    }
}

/// A title row wrapped at words, its rows under its own indent: a long command stays readable.
fn title_rows(line: &Line<'static>, width: usize) -> Vec<Line<'static>> {
    let mut body = line.clone();
    let mut indent = String::new();
    while let Some(first) = body.spans.first_mut() {
        let text = first.content.to_string();
        let kept = text.trim_start();
        indent.push_str(text.strip_suffix(kept).unwrap_or_default());
        if kept.is_empty() {
            body.spans.remove(0);
        } else {
            first.content = kept.to_string().into();
            break;
        }
    }
    let prefix = Line::from(indent);
    wrap(&body, width, &prefix, &prefix)
}
