//! The window's three states on macOS (ARCHITECTURE.md A9) as pure logic: events in, what to do
//! out. Hidden while no tab is open; the mini, a small floating preview, whenever one is; the
//! expanded window, which only the person's click opens. The agent's acts spin the mini and
//! refresh its picture, its `show` blinks it, and nothing it does expands anything. Where there
//! is no mini ([`Support::WindowOnly`]) the window opens as an ordinary one. The AppKit side
//! (host_presence.rs) applies what this decides; the timings come in as values.

use std::time::Duration;

/// Where the window is, as the person sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    Hidden,
    Mini,
    Expanded,
}

/// Whether this system has a mini to collapse to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Support {
    Mini,
    WindowOnly,
}

/// How long things last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Timing {
    /// The mini's highlight when the agent shows a tab.
    pub blink: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            blink: Duration::from_millis(700),
        }
    }
}

/// What happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Event {
    /// A window with tabs is on screen, where there was none.
    Appeared,
    /// Its last tab closed.
    Vanished,
    /// The person clicked the mini.
    Expand,
    /// The person pressed the window's minimize button.
    Collapse,
    /// The agent began an action.
    Acting,
    /// The agent's action settled.
    Settled,
    /// The agent showed a tab.
    Show,
    /// What the mini shows changed (the tab shown, a page loaded).
    Changed,
    /// A picture asked for has come (or failed).
    Pictured,
}

/// What to do about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Effect {
    /// Put the windows as this state says.
    Enter(State),
    /// Take the shown tab's picture for the mini.
    Capture,
    /// The mini's spinner on or off.
    Busy(bool),
    /// The mini's highlight, for this long.
    Blink(Duration),
}

/// Whether the agent is in the middle of an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Agent {
    Idle,
    Acting,
}

/// The mini's picture: one is taken at a time, and a change meanwhile asks for one more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Picture {
    Idle,
    Taking,
    Stale,
}

pub(super) struct Presence {
    state: State,
    support: Support,
    timing: Timing,
    agent: Agent,
    picture: Picture,
}

impl Presence {
    pub(super) fn new(support: Support, timing: Timing) -> Self {
        Self {
            state: State::Hidden,
            support,
            timing,
            agent: Agent::Idle,
            picture: Picture::Idle,
        }
    }

    pub(super) fn state(&self) -> State {
        self.state
    }

    /// What `event` makes happen.
    pub(super) fn event(&mut self, event: Event) -> Vec<Effect> {
        match event {
            Event::Appeared => self.appear(),
            Event::Vanished => self.vanish(),
            Event::Expand => self.enter(State::Mini, State::Expanded),
            Event::Collapse if self.support == Support::Mini => {
                let mut out = self.enter(State::Expanded, State::Mini);
                out.extend(self.refresh());
                out
            }
            Event::Collapse => vec![],
            Event::Acting => self.agent(Agent::Acting),
            Event::Settled => {
                let mut out = self.agent(Agent::Idle);
                out.extend(self.refresh());
                out
            }
            Event::Show if self.state == State::Mini => {
                let mut out = vec![Effect::Blink(self.timing.blink)];
                out.extend(self.refresh());
                out
            }
            Event::Show => vec![],
            Event::Changed => self.refresh(),
            Event::Pictured => match self.picture {
                Picture::Stale => {
                    self.picture = Picture::Taking;
                    vec![Effect::Capture]
                }
                Picture::Idle | Picture::Taking => {
                    self.picture = Picture::Idle;
                    vec![]
                }
            },
        }
    }

    fn appear(&mut self) -> Vec<Effect> {
        if self.state != State::Hidden {
            return vec![];
        }
        match self.support {
            Support::Mini => {
                self.state = State::Mini;
                let mut out = vec![Effect::Enter(State::Mini)];
                out.extend(self.refresh());
                out
            }
            Support::WindowOnly => {
                self.state = State::Expanded;
                vec![Effect::Enter(State::Expanded)]
            }
        }
    }

    fn vanish(&mut self) -> Vec<Effect> {
        if self.state == State::Hidden {
            return vec![];
        }
        self.state = State::Hidden;
        self.agent = Agent::Idle;
        self.picture = Picture::Idle;
        vec![Effect::Enter(State::Hidden)]
    }

    /// `to` when the window is `from`, else nothing.
    fn enter(&mut self, from: State, to: State) -> Vec<Effect> {
        if self.state != from {
            return vec![];
        }
        self.state = to;
        vec![Effect::Enter(to)]
    }

    fn agent(&mut self, now: Agent) -> Vec<Effect> {
        if self.state == State::Hidden || self.agent == now {
            return vec![];
        }
        self.agent = now;
        vec![Effect::Busy(now == Agent::Acting)]
    }

    /// A new picture of what the mini shows, when it is the mini that shows it.
    fn refresh(&mut self) -> Vec<Effect> {
        if self.state != State::Mini {
            return vec![];
        }
        match self.picture {
            Picture::Idle => {
                self.picture = Picture::Taking;
                vec![Effect::Capture]
            }
            Picture::Taking => {
                self.picture = Picture::Stale;
                vec![]
            }
            Picture::Stale => vec![],
        }
    }
}

#[cfg(test)]
#[path = "presence_tests.rs"]
mod tests;
