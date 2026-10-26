//! The browser's own commands on its tabs in the window: a script, a load, a picture, a size,
//! and a tab opened, switched to or closed.

use std::sync::Arc;

use tao::event_loop::EventLoopProxy;

use super::super::capture::capture;
use super::super::deck::{Deck, Shown};
use super::{Cmd, Shared, WINDOW_CLOSED};

/// The browser is closed: a script waiting on it fails now.
fn closed(command: &Cmd, shared: &Shared) {
    if let Cmd::Eval { id, .. } = command {
        shared.answer(*id, Err(WINDOW_CLOSED.into()));
    }
}

/// A command of the browser's own, on its tabs in the window.
pub(super) fn on_browser(
    command: Cmd,
    deck: &mut Option<Deck>,
    shared: &Arc<Shared>,
    proxy: &EventLoopProxy<Cmd>,
) {
    // A driver exists only once the browser opened: with none now, it was closed.
    let Some(open) = deck.as_mut() else {
        return closed(&command, shared);
    };
    let Some(tabs) = open.web.as_mut() else {
        return closed(&command, shared);
    };
    match command {
        Cmd::Eval {
            id,
            script,
            failed,
            tab,
            ..
        } => {
            let answer = Arc::clone(shared);
            let sent = tabs
                .at(tab)
                .evaluate_script_with_callback(&script, move |started| {
                    if started.trim() != "true" {
                        answer.answer(id, Err(failed.into()));
                    }
                });
            if let Err(error) = sent {
                shared.answer(id, Err(format!("{failed}: {error}")));
            }
            return;
        }
        Cmd::Resize(width, height) => open.resize(width, height),
        Cmd::Capture { clip, reply, tab } => return capture(tabs.at(tab), clip, reply),
        Cmd::Noted(noted) => {
            let (reply, shot) = std::sync::mpsc::sync_channel(1);
            if noted.clip.is_some() {
                capture(tabs.at(Some(noted.tab)), noted.clip, reply);
            }
            return crate::browser::comments::pictured(noted, shot, shared, proxy);
        }
        // What the agent does in the browser is shown: the window goes back to it, unless it
        // works in a tab of its own beside the one shown.
        // An address the system cannot read never reaches WebKit: its load would panic here.
        Cmd::Navigate(url, tab) => {
            if let Some(url) = crate::browser::guard::loadable(&url) {
                let _ = tabs.at(tab).load_url(&url);
            }
            if tab.is_none() {
                open.show(Shown::Web);
            }
        }
        Cmd::NewTab { url, reply } => {
            let added = match crate::browser::guard::loadable(&url) {
                Some(url) => tabs.add(&open.frame.native, &url, shared, proxy),
                None => Err(format!("`{url}` is not an address")),
            };
            match reply {
                Some(reply) => {
                    let _ = reply.send(added);
                }
                // A page's window past the tab limit opens where the page is, as before tabs.
                None if added.is_err() => {
                    if let Some(url) = crate::browser::guard::loadable(&url) {
                        let _ = tabs.active().load_url(&url);
                    }
                }
                None => {}
            }
            open.show(Shown::Web);
        }
        Cmd::SwitchTab { id, reply } => {
            let _ = reply.send(tabs.switch(id, shared));
            open.show(Shown::Web);
        }
        Cmd::CloseTab { id, reply } => {
            let closed = tabs.close(id, shared);
            let emptied = closed == Ok(false);
            let _ = reply.send(closed);
            if emptied {
                shared.closed();
                if !open.remove_web() {
                    *deck = None;
                    return;
                }
            }
        }
        Cmd::Open { .. }
        | Cmd::Close
        | Cmd::Panel(_)
        | Cmd::Paint(_)
        | Cmd::Unpanel(_)
        | Cmd::Titled(..)
        | Cmd::Clicked(_)
        | Cmd::Strip
        | Cmd::Dressed(_)
        | Cmd::Listing(_)
        | Cmd::Sized(..)
        | Cmd::Act(..)
        | Cmd::Exit(_) => return,
    }
    open.refresh(shared);
}
