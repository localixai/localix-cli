//! The composer drawn: its rows after the prompt at a width, and where the cursor is among them.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;

use super::Composer;
use crate::tui::style;
use crate::tui::wrap::width as columns;

impl Composer {
    /// The rows the composer takes at `width`, after `prompt`, and where its cursor is among them.
    /// The text breaks at the width wherever it reaches it, so every character keeps its column.
    pub(crate) fn render(
        &self,
        prompt: &Span<'static>,
        placeholder: &str,
        width: usize,
    ) -> (Vec<Line<'static>>, (usize, usize)) {
        let indent = columns(&prompt.content);
        let room = width.saturating_sub(indent).max(1);
        let mut rows: Vec<Vec<Span<'static>>> = vec![Vec::new()];
        let selection = self.selection();
        let mut used = 0;
        let mut cursor = (0, indent);
        for (index, c) in self.text.iter().enumerate() {
            if *c == '\n' {
                if index == self.at {
                    cursor = (rows.len() - 1, indent + used);
                }
                rows.push(Vec::new());
                used = 0;
                continue;
            }
            let w = c.width().unwrap_or(0);
            if used + w > room {
                rows.push(Vec::new());
                used = 0;
            }
            if index == self.at {
                cursor = (rows.len() - 1, indent + used);
            }
            if let Some(row) = rows.last_mut() {
                let selected = selection
                    .as_ref()
                    .is_some_and(|range| range.contains(&index));
                let appearance = if selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                let character = if self.masked { '•' } else { *c };
                if let Some(last) = row.last_mut().filter(|span| span.style == appearance) {
                    last.content.to_mut().push(character);
                } else {
                    row.push(Span::styled(character.to_string(), appearance));
                }
            }
            used += w;
        }
        if self.at == self.text.len() {
            if used >= room {
                rows.push(Vec::new());
                used = 0;
            }
            cursor = (rows.len() - 1, indent + used);
        }
        let pad = " ".repeat(indent);
        let lines = rows
            .into_iter()
            .enumerate()
            .map(|(at, row)| {
                let lead = if at == 0 {
                    prompt.clone()
                } else {
                    Span::raw(pad.clone())
                };
                match (self.text.is_empty(), at) {
                    (true, 0) => Line::from(vec![
                        lead,
                        Span::styled(placeholder.to_string(), style::dim()),
                    ]),
                    _ => Line::from(std::iter::once(lead).chain(row).collect::<Vec<_>>()),
                }
            })
            .collect();
        (lines, cursor)
    }
}
