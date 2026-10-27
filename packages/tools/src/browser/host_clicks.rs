//! What the loop does with the person's clicks and the window's presence: a tab shown or closed,
//! the history menu's entry, a page's answer, the mini named, its picture, an event of its state.

use std::sync::Arc;

use serde_json::Value;

use super::super::presence::Stage;
use super::super::{Cmd, Heard, MINI, Shared, WINDOW_CLOSED};
use super::On;
use crate::browser::capture::Shot;
use crate::browser::deck::{After, Deck, Revisit};
use crate::browser::presence::Event as Presence;
use crate::browser::strip::{Click, Key, click};

impl On<'_> {
    /// The mini names the tab shown again: its project may be known by now.
    pub(super) fn renamed(&mut self) {
        let stage = Stage {
            deck: &mut *self.deck,
            target: self.target,
            shared: self.shared,
            proxy: self.proxy,
        };
        self.presenter.rename(&stage);
    }

    pub(super) fn pictured(&mut self, shot: &Result<Shot, String>) {
        let mut stage = Stage {
            deck: &mut *self.deck,
            target: self.target,
            shared: self.shared,
            proxy: self.proxy,
        };
        self.presenter.pictured(shot, &mut stage);
    }

    /// `event` for the window's presence.
    pub(super) fn presence(&mut self, event: Presence) {
        let mut stage = Stage {
            deck: &mut *self.deck,
            target: self.target,
            shared: self.shared,
            proxy: self.proxy,
        };
        self.presenter.event(event, &mut stage);
    }

    /// The person's click: closing the browser's last tab is closing its window.
    pub(super) fn clicked(&mut self, said: &str) {
        match click(said) {
            Some(Click::Visit(at)) => return self.visit(at),
            Some(Click::Collapse) => return self.presence(Presence::Collapse),
            _ => {}
        }
        if let Some(said) = click(said)
            && act(self.deck, said, self.shared, Some(self.on_close)).is_ok()
            && let Click::Close(key) = said
        {
            let what = match key {
                Key::Web(_) => CLOSED_PAGE,
                Key::Term(_) => CLOSED_TERMINAL,
            };
            self.shared.heard(&key.name(), Heard::Did(what));
        }
    }

    /// An entry of the history menu shown again; a closed page opens in a tab of its own.
    fn visit(&mut self, at: u64) {
        let entry = self.shared.history.get(at);
        if let (Some(entry), Some(open)) = (entry, self.deck.as_mut()) {
            if let Revisit::Load(url) = open.revisit(&entry, self.shared) {
                let _ = self.proxy.send_event(Cmd::NewTab { url, reply: None });
            }
            open.refresh(self.shared);
        }
    }

    /// `script` in a panel's page, or in the strip's (`STRIP`, for checks).
    pub(super) fn eval_panel(&self, id: u64, script: &str, panel: u32) {
        let answer = Arc::clone(self.shared);
        let reply = move |value: String| {
            let value = serde_json::from_str(&value).unwrap_or(Value::String(value));
            answer.answer(id, Ok(value));
        };
        let page = match panel {
            MINI => self.presenter.page(),
            panel => self.deck.as_ref().and_then(|open| open.page(panel)),
        };
        match page {
            Some(page) => drop(page.evaluate_script_with_callback(script, reply)),
            None => self.shared.answer(id, Err(WINDOW_CLOSED.into())),
        }
    }
}

/// Show or close the tab `said` names. A person's click closing the browser's last tab stops the
/// task as closing its window does (`on_close`); the agent's goes on.
pub(super) fn act(
    deck: &mut Option<Deck>,
    said: Click,
    shared: &Shared,
    on_close: Option<fn()>,
) -> Result<(), String> {
    let open = deck.as_mut().ok_or("the window is not open")?;
    let after = open.act(said, shared)?;
    if let (After::BrowserGone { .. }, Some(on_close)) = (&after, on_close) {
        on_close();
    }
    match after {
        After::Empty | After::BrowserGone { empty: true } => *deck = None,
        After::Kept | After::BrowserGone { .. } => open.refresh(shared),
    }
    Ok(())
}

/// What the person's closing says, to whoever listens.
const CLOSED_PAGE: &str = "The person closed this browser tab.";
const CLOSED_TERMINAL: &str = "The person closed this tab: its program runs on, out of sight.";
