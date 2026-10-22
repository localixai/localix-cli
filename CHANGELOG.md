# Changelog

## Unreleased

lxsurf is rebuilt as a small agent and its hands, in layered packages. Breaking: the `--json` schema is 3, and `--url` and `--viewport` are gone: a goal names where it starts, the agent opens addresses and changes the page size.

- **An agent and its hands.** The chat model (`--model`, default `anthropic/claude-haiku-5.5`) is an agent with one tool, `browser`: open an address, hand a short task with the exact `values` to type to the hands, press keys, run a script, read the whole text. The hands are the decision loop: the decision model (`--decision-model`, default `~typesafe/jev-latest`) carries a task out step by step. The agent checks every page itself and answers with a `RESULT:` line.
- **Packages.** `contracts` (ports only: chat model, the agent's tools, the view, the decision model, the hands, history), `agent`, `decision-loop` (the hands), `tools` (the browser), `openrouter` (both models' provider), `session`, `apps/cli` (the `lxsurf` binary). Dependencies point inward; `apps/cli/tests/architecture.rs` checks it, and that unit tests live in their own `x_tests.rs` files.
- **No site hints.** The agent's prompt and the tool's description are general: how to phrase a task (one page's worth, actions only, how to tell it is done), what the hands can and cannot do. Tool results come in sections (`### Helper`, `### Page (untrusted content)`), the page as an outline without refs.
- **Kept short.** A tool result's report stays for the whole conversation; its state (the page) only until the conversation passes its budget; a result too long keeps its head and tail. Calls left unanswered are closed before a follow-up.
- **Stops cleanly.** A failed call goes back to the model to try another way; after three in a row, or on the last turn, it is asked to answer with what it has. A fatal one (the decision model unreachable) ends the run.
- **`--schema`** still returns the answer as JSON matching a JSON Schema through the `result` tool, checked before it is printed. **`--no-llm`** now runs the hands alone on the whole goal, from the page it names, typing quoted strings or words of the goal. `--max-steps` bounds one task of the hands (40).
- **`--json` schema 3.** `usage` has `agent` (with `cached_tokens`) and `decision`; `data` / `data_error` with `--schema`; status `unstated` when the agent does not say; `tool` is gone; `final_url` and `screenshot`.
- **Keys again.** `browser` presses keys (`keys`, `key_delay_ms`) with the `key`, `code` and `keyCode` games read.
- **The page as an outline.** The hands read the page as an outline: headings and text in reading order, every control with its ref, value and state, by structure. `--dry-run` and `/snapshot` print it. The browser's toolbar (Back, Reload, page size) is part of the view.
- **Wait, as long as needed.** WAIT carries a length the decision model picks (1 to 30 s), ending early when the page changes.
- **Screenshots.** `--screenshot FILE` saves a PNG of the final page, and `/screenshot` in the chat; WebKit's own snapshot, in an invisible window too (macOS).
- **Chat and sessions.** `--keep-open` leaves a chat: each line is the next goal in the same window and conversation, or `/snapshot`, `/screenshot [FILE]`. Every run is a session (`lxsurf sessions`, `-c`, `--resume ID`). `--timeout` bounds each goal.
- **Steadier on real sites:**
  - a filled field and a placed value are not offered again;
  - consent banners in same-origin frames are seen and accepted; a click under one that loads late is decided again;
  - a script or a click that leaves the page no longer waits for a reply that cannot come, and a parsed document is used after at most 1 s more instead of waiting up to 20 s for every ad and video;
  - with the Navigation API reporting the page's own navigations, actions on ad-heavy pages no longer wait 300 ms for frames' navigations;
  - the page's own requests in flight mark it busy; an unsure DONE, or one while fetching, is decided again only if the page changes;
  - the step history is numbered, marks steps that opened a new view, and counts ("three questions") hold;
  - a link inside a heading (a search result's title, as on DuckDuckGo) keeps its ref in the outline instead of showing only as the heading's words.
- A `<button>` is named by its text, not its `value`; a refused action shows why in the trace. The planner no longer sends `temperature`, which current Claude models refuse.

## 0.1.0

First release, as a browser CLI with a chat model that planned and a decision model that clicked and typed: `check` and `--expect`, `--json` and `--trace`, stable exit codes, headless by default under CI, OpenRouter sign-in, viewports, cost reports, key presses, page scripts, date pickers, guides for agents.
