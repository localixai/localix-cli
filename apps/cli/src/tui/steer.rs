//! Steering: what the person sends while a task works goes to that task at its next step;
//! a slash command, a picture, or Tab waits for the task to end instead. The task
//! takes what was sent from here between its turns, and only then is it shown as the person's.

use std::sync::mpsc::Sender;

use super::app::App;
use super::cells;
use super::event::Typed;

/// A task as the transcript shows it: what came back from the background reads as where it came
/// from (`≋ ticks · tick 1`, `◷ hello · say hello`), not as the tags the model reads, and `/init`
/// as itself.
pub fn shown(goal: &str) -> String {
    // `/init` is sent as its instruction, and reads as what the person typed.
    if goal == crate::commands::extras::INIT {
        return "/init".into();
    }
    let goal = goal.strip_prefix(crate::jobs::FROM_MONITOR).unwrap_or(goal);
    if let Some(rest) = goal.strip_prefix("<monitor job=\"")
        && let Some((name, body)) = rest.split_once("\">\n")
    {
        let body = body.trim_end().trim_end_matches("</monitor>").trim_end();
        return format!("≋ {name} · {body}");
    }
    if let Some(rest) = goal.strip_prefix("(scheduled `")
        && let Some((name, prompt)) = rest.split_once("`) ")
    {
        return format!("◷ {name} · {prompt}");
    }
    goal.to_string()
}

/// Whether `entry` may go to the task running: words only.
pub fn steerable(entry: &Typed) -> bool {
    let text = entry.text.trim_start();
    entry.images.is_empty()
        && !text.is_empty()
        && !text.starts_with('/')
        && !text.starts_with('\u{1b}')
}

impl App {
    /// Give the task what was sent to it (shown now as the person's), or say whether there is
    /// any (`peek`: the texts, left in place).
    pub(super) fn take_steered(&mut self, at: usize, reply: &Sender<Vec<String>>, peek: bool) {
        let Some(lane) = self.views.sessions[at].lane.as_deref_mut() else {
            return;
        };
        let texts: Vec<String> = lane.steered.iter().map(|e| e.text.clone()).collect();
        if !peek {
            lane.steered.clear();
            for text in &texts {
                let block = cells::user(text, self.width);
                self.push(at, block);
            }
        }
        let _ = reply.send(texts);
    }

    /// The composer's text for after the task: Tab while it works.
    pub(super) fn queue_for_later(&mut self) {
        let entry = self.composer.text();
        if entry.trim().is_empty() {
            return;
        }
        self.composer.clear();
        let (text, images) = self.composer.take_attached(&entry);
        self.lane_mut().queued.push_back(Typed { text, images });
    }
}

#[cfg(test)]
#[path = "steer_tests.rs"]
mod tests;
