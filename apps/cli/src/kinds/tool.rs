//! The `window` tool over its kinds, built the one way for every client of the server: a
//! session's agent and its agents (`agents::kit`), and a tool client (`server::toolbox`).

use contracts::gate::Gate;
use contracts::hands::Hands;
use tools::browser::Page;
use tools::shell::{BashTool, Sessions};
use tools::window::{Kind, WindowTool, Windowed};

use super::BrowserLocks;

/// What the `window` tool is built from: the terminal sessions (through `bash`), the browser's
/// page where there is one, the window the person watches, the hands and the call's gate.
pub(crate) struct Parts<'a> {
    pub bash: &'a BashTool<'a>,
    pub sessions: &'a Sessions,
    pub stop: &'a (dyn Fn() -> bool + Sync),
    pub page: Option<&'a Page>,
    pub locks: &'a BrowserLocks,
    pub windowed: Option<&'a dyn Windowed>,
    pub hands: &'a dyn Hands,
    pub gate: &'a dyn Gate,
}

/// `work` with the `window` tool built from `parts`: terminal tabs where there are terminal
/// sessions (unix), and browser tabs where there is a page (else a browser kind that says why
/// there is none).
pub(crate) fn window<T>(parts: &Parts, work: impl FnOnce(Option<&WindowTool>) -> T) -> T {
    let browser = (parts.page).map(|page| super::Browser {
        page,
        gate: parts.gate,
        locks: parts.locks,
        workspace: parts.bash.workspace,
    });
    #[cfg(unix)]
    let terminal = super::Terminal {
        bash: parts.bash,
        sessions: parts.sessions,
        stop: parts.stop,
    };
    let mut kinds: Vec<&dyn Kind> = vec![];
    #[cfg(unix)]
    kinds.push(&terminal);
    // Without a page, the browser kind is still there, to say why a page cannot open.
    match &browser {
        Some(browser) => kinds.push(browser),
        None => kinds.push(&super::Missing),
    }
    let window = WindowTool {
        kinds,
        window: parts.windowed,
        hands: Some(parts.hands),
        gate: parts.gate,
    };
    work(Some(&window).filter(|window| !window.kinds.is_empty()))
}
