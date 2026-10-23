//! Choosing at the terminal: a title and options in, the index chosen out, or nothing when the
//! person cancels ([`Picker`]); and a line of text, hidden for a secret ([`Lines`]). The
//! interface implements both (`tui::asking::UiTerminal`).

/// One option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Item {
    pub label: String,
    /// A short note after the label.
    pub detail: String,
    /// The option in effect now (or the default): Enter alone picks it.
    pub marked: bool,
}

impl Item {
    pub(crate) fn new(label: impl Into<String>, detail: impl Into<String>, marked: bool) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
            marked,
        }
    }
}

/// One row's inline values and the value initially selected.
pub(crate) use protocol::Adjustment;

pub(crate) trait Picker {
    /// The index of the option chosen; `None` when the person cancels.
    fn pick(&mut self, title: &str, items: &[Item]) -> Option<usize>;

    /// The row and its selected value; non-interactive pickers keep the initial value.
    fn pick_adjustable(
        &mut self,
        title: &str,
        items: &[Item],
        adjustments: &[Adjustment],
    ) -> Option<(usize, usize)> {
        let at = self.pick(title, items)?;
        Some((
            at,
            adjustments.get(at).map_or(0, |adjustment| adjustment.at),
        ))
    }
}

pub(crate) trait Lines {
    /// The line typed after `prompt`, not shown on the screen when `hidden`; `None` at the end
    /// of input.
    fn line(&mut self, prompt: &str, hidden: bool) -> Option<String>;

    /// [`Lines::line`], closed unanswered (`None`) once `done` says so: a sign-in's callback
    /// came back while the person could still paste.
    fn line_until(&mut self, prompt: &str, done: &dyn Fn() -> bool) -> Option<String> {
        let _ = done;
        self.line(prompt, false)
    }
}

/// What a sign-in flow needs at the terminal.
pub(crate) trait Terminal: Picker + Lines {}

impl<T: Picker + Lines> Terminal for T {}
