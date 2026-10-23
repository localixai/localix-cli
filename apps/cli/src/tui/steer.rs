//! Steering: what the person sends while a task works goes to that task at its next step; a
//! slash command, a picture, or Tab waits for the task to end instead. The server decides which
//! (`server/inputs.rs`); the interface only says when an entry is for after the task.

use protocol::{Command, Input};

use super::app::App;
use super::event::Out;

impl App {
    /// The composer's text for after the task: Tab while it works.
    pub(super) fn queue_for_later(&mut self) {
        let entry = self.composer.text();
        if entry.trim().is_empty() {
            return;
        }
        self.composer.clear();
        let (text, images) = self.composer.take_attached(&entry);
        let session = self.lane_id().to_string();
        let input = Input {
            session,
            text,
            images,
            later: true,
        };
        self.outbox.push(Out::Call(Command::Input(input)));
    }
}
