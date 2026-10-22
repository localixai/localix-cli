//! Choosing at the terminal: a title and options in, the index chosen out, or nothing when the
//! person cancels ([`Picker`]); and a line of text, hidden for a secret ([`Lines`]). [`Numbered`]
//! is the plain fallback: a numbered list read from the prompt line. An arrow-key picker
//! implements the same trait.

use std::io::BufRead;

/// One option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub label: String,
    /// A short note after the label.
    pub detail: String,
    /// The option in effect now (or the default): Enter alone picks it.
    pub marked: bool,
}

impl Item {
    pub fn new(label: impl Into<String>, detail: impl Into<String>, marked: bool) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
            marked,
        }
    }
}

pub trait Picker {
    /// The index of the option chosen; `None` when the person cancels.
    fn pick(&mut self, title: &str, items: &[Item]) -> Option<usize>;
}

pub trait Lines {
    /// The line typed after `prompt`, not shown on the screen when `hidden`; `None` at the end
    /// of input.
    fn line(&mut self, prompt: &str, hidden: bool) -> Option<String>;
}

/// What a sign-in flow needs at the terminal.
pub trait Terminal: Picker + Lines {}

impl<T: Picker + Lines> Terminal for T {}

/// A numbered list on stderr, its answer read from `input`.
pub struct Numbered<'a> {
    pub input: &'a mut dyn BufRead,
    pub shown: &'a dyn Fn(&str),
}

impl Picker for Numbered<'_> {
    fn pick(&mut self, title: &str, items: &[Item]) -> Option<usize> {
        let mut list = format!("\n{title}\n");
        for (at, item) in items.iter().enumerate() {
            let mark = if item.marked { "*" } else { " " };
            list.push_str(&format!("  {mark}{:>2}. {}", at + 1, item.label));
            if !item.detail.is_empty() {
                list.push_str(&format!("  {}", item.detail));
            }
            list.push('\n');
        }
        (self.shown)(&list);
        let marked = items.iter().position(|item| item.marked);
        let hint = match marked {
            Some(at) => format!("Number (Enter: {}, q: cancel) > ", at + 1),
            None => "Number (q: cancel) > ".into(),
        };
        loop {
            let answer = self.line(&hint, false)?;
            match chosen(&answer, items.len(), marked) {
                Answer::Chosen(at) => return Some(at),
                Answer::Cancel => return None,
                Answer::Again => (self.shown)(&format!("  1 to {}, or q\n", items.len())),
            }
        }
    }
}

impl Lines for Numbered<'_> {
    fn line(&mut self, prompt: &str, hidden: bool) -> Option<String> {
        (self.shown)(prompt);
        let mut bytes = Vec::new();
        let read = match hidden {
            true => unseen(|| self.input.read_until(b'\n', &mut bytes)),
            false => self.input.read_until(b'\n', &mut bytes),
        };
        match read {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(String::from_utf8_lossy(&bytes).trim().to_string()),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
    Chosen(usize),
    Cancel,
    Again,
}

/// What `answer` picks among `count` options, Enter picking `marked`.
pub fn chosen(answer: &str, count: usize, marked: Option<usize>) -> Answer {
    match answer.trim().to_ascii_lowercase().as_str() {
        "" => marked.map_or(Answer::Again, Answer::Chosen),
        "q" | "quit" | "cancel" => Answer::Cancel,
        number => match number.parse::<usize>() {
            Ok(n) if (1..=count).contains(&n) => Answer::Chosen(n - 1),
            _ => Answer::Again,
        },
    }
}

/// Run `read` with the terminal's echo off, so a pasted key never stays on the screen or in its
/// scrollback. Ctrl-C is read as a character meanwhile, so the process is never killed with the
/// echo still off; the terminal is put back as it was either way.
#[cfg(unix)]
fn unseen<T>(read: impl FnOnce() -> std::io::Result<T>) -> std::io::Result<T> {
    use std::mem::MaybeUninit;
    let fd = libc::STDIN_FILENO;
    let mut saved = MaybeUninit::<libc::termios>::uninit();
    // SAFETY: tcgetattr fills `saved` for a terminal; it is read only when that succeeded.
    if unsafe { libc::tcgetattr(fd, saved.as_mut_ptr()) } != 0 {
        return read();
    }
    let saved = unsafe { saved.assume_init() };
    let mut quiet = saved;
    quiet.c_lflag &= !(libc::ECHO | libc::ISIG);
    quiet.c_lflag |= libc::ECHONL;
    // SAFETY: both settings are whole termios values for this descriptor.
    unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &quiet) };
    let result = read();
    unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &saved) };
    result
}

/// Windows: the console keeps its echo (no std call turns it off).
#[cfg(not(unix))]
fn unseen<T>(read: impl FnOnce() -> std::io::Result<T>) -> std::io::Result<T> {
    read()
}

#[cfg(test)]
#[path = "picker_tests.rs"]
mod tests;
