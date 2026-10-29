//! A request as the Responses API takes it: the conversation as input items, the tools, the
//! level, and the prompt cache's key.

use contracts::catalog::{Effort, Selection};
use contracts::chat::{Message, Request};
use serde_json::{Value, json};

use super::offer::{Offer, Texts};

/// The request body for `request`, a tool of free text offered as `texts` says.
pub(crate) fn body(selection: &Selection, request: Request, texts: Texts) -> Value {
    let offer = Offer::new(request.tools, texts, request.conversation);
    let mut body = json!({
        "model": selection.model().id,
        "input": (request.conversation.iter())
            .flat_map(|message| items(message, &selection.model().id, &offer))
            .collect::<Vec<_>>(),
        "store": false,
        "stream": true,
        "include": [],
        "prompt_cache_key": super::route::session(request.conversation),
    });
    // Off is said: a request without the field reasons at the backend's own default.
    if !selection.model().efforts.is_empty() {
        let effort = selection.effort();
        let name = if effort == Effort::Off {
            "none"
        } else {
            effort.name()
        };
        body["reasoning"] = json!({"effort": name});
        // The reasoning comes back sealed, to be sent with the next request (`items`).
        body["include"] = json!(["reasoning.encrypted_content"]);
    }
    if !request.tools.is_empty() {
        body["tools"] = request.tools.iter().map(|spec| offer.tool(spec)).collect();
        body["tool_choice"] = json!(if request.may_call { "auto" } else { "none" });
        // Several calls in one turn: independent reads and searches cost one turn, not many. The
        // agent runs together only the calls whose tools allow it, the rest in order.
        body["parallel_tool_calls"] = json!(true);
    }
    body
}

/// A message as the Responses API's input items, for `model`: a reasoning another model sealed
/// (before `/model` switched) is left out, as that model alone can open it.
pub(super) fn items(message: &Message, model: &str, offer: &Offer) -> Vec<Value> {
    let text = |role: &str, kind: &str, text: &str| json!({"type": "message", "role": role, "content": [{"type": kind, "text": text}]});
    match message {
        Message::System(rules) => vec![text("developer", "input_text", rules)],
        Message::User(said) => vec![text("user", "input_text", said)],
        Message::Assistant {
            text: said,
            calls,
            carried,
        } => {
            // Only what this provider kept: another's (chat completions' fields) has no "item".
            let kept: Vec<Value> = (carried.iter())
                .filter(|kept| kept["model"] == model)
                .filter_map(|kept| kept.get("item").cloned())
                .collect();
            // The text goes back as the messages it came in, with their phase, when they were kept.
            let whole = kept.iter().any(|item| item["type"] == "message");
            let said = said.iter().filter(|_| !whole);
            (kept.into_iter())
                .chain(said.map(|said| text("assistant", "output_text", said)))
                .chain(calls.iter().map(|call| offer.call_item(call)))
                .collect()
        }
        // A picture goes in as the person's message, after the call that took it.
        Message::Image { caption, image } => vec![json!({"type": "message", "role": "user",
            "content": [{"type": "input_text", "text": caption},
                {"type": "input_image",
                 "image_url": format!("data:{};base64,{}", image.media_type, image.data)}]})],
        // The wire has no failure flag for a tool result: the text says it.
        Message::ToolResult {
            id,
            content,
            failed,
        } => {
            let output = match failed {
                true => format!("Error: {content}"),
                false => content.clone(),
            };
            vec![offer.output_item(id, &output)]
        }
    }
}
