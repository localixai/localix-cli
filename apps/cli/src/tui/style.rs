//! The palette and how styled lines reach a terminal. Colour carries meaning only: dim for what is
//! secondary, bold for what the person typed, green for done, red for failed, yellow for what
//! waits on the person, cyan for code. The 16 ANSI colours, so the terminal's theme picks the
//! shades. A line goes out as ANSI, or as plain text where colour is off.

use std::io::IsTerminal;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;

pub fn dim() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}
pub fn bold() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}
pub fn done() -> Style {
    Style::new().fg(Color::Green)
}
pub fn failed() -> Style {
    Style::new().fg(Color::Red)
}
pub fn waiting() -> Style {
    Style::new().fg(Color::Yellow)
}
pub fn code() -> Style {
    Style::new().fg(Color::Cyan)
}

/// Whether colour goes to a stream: `CLICOLOR_FORCE` wins, then `NO_COLOR` and `TERM=dumb` turn it
/// off, then a terminal turns it on.
pub fn colour(terminal: bool, var: &dyn Fn(&str) -> Option<String>) -> bool {
    let forced = var("CLICOLOR_FORCE").is_some_and(|v| !v.is_empty() && v != "0");
    let off = var("NO_COLOR").is_some() || var("TERM").as_deref() == Some("dumb");
    forced || (terminal && !off)
}

/// Colour for stderr, where the trace goes.
pub fn colour_stderr() -> bool {
    colour(std::io::stderr().is_terminal(), &|name| {
        std::env::var(name).ok()
    })
}

/// Colour for stdout, where the answer goes.
pub fn colour_stdout() -> bool {
    colour(std::io::stdout().is_terminal(), &|name| {
        std::env::var(name).ok()
    })
}

/// `line` as the terminal draws it: SGR codes around each styled span, or plain text.
pub fn ansi(line: &Line, colour: bool) -> String {
    let mut out = String::new();
    for span in &line.spans {
        let style = line.style.patch(span.style);
        let codes = if colour { sgr(style) } else { String::new() };
        if codes.is_empty() {
            out.push_str(&span.content);
        } else {
            out.push_str(&format!("\x1b[{codes}m{}\x1b[0m", span.content));
        }
    }
    out
}

/// `line` without its styles.
#[cfg(test)]
pub fn plain(line: &Line) -> String {
    ansi(line, false)
}

fn sgr(style: Style) -> String {
    let mut codes: Vec<&str> = vec![];
    let modifier = style.add_modifier - style.sub_modifier;
    for (flag, code) in [
        (Modifier::BOLD, "1"),
        (Modifier::DIM, "2"),
        (Modifier::ITALIC, "3"),
        (Modifier::UNDERLINED, "4"),
        (Modifier::REVERSED, "7"),
    ] {
        if modifier.contains(flag) {
            codes.push(code);
        }
    }
    let fg = style.fg.and_then(|colour| match colour {
        Color::Red => Some("31"),
        Color::Green => Some("32"),
        Color::Yellow => Some("33"),
        Color::Blue => Some("34"),
        Color::Magenta => Some("35"),
        Color::Cyan => Some("36"),
        Color::Gray => Some("37"),
        Color::DarkGray => Some("90"),
        _ => None,
    });
    codes.extend(fg);
    codes.join(";")
}

#[cfg(test)]
#[path = "style_tests.rs"]
mod tests;
