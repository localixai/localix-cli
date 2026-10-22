# Changelog

## Unreleased: Decision Loop

The project is now Decision Loop: a goal driven to its end by a decision model alone, through tools. The browser CLI became its first tool. Breaking: the binary, flags, environment variables, the `--json` schema and the data folders are new.

- **No chat model.** The planner, its tools, `--schema`, `--no-llm`, `--max-turns`, `LXSURF_MODEL` and the chat-model checks in `doctor` and `models` are gone. Every step is one decision-model request; the run ends on DONE or BLOCKED.
- **The decision loop.** A goal run stops on its guards (the same action twice without a change, a loop, a refused action, its share of actions); the loop resumes it with its whole history until DONE or BLOCKED, the action budget (`--max-steps`, default 100) is spent, or two runs in a row stop without progress. A 25-question quiz is one goal.
- **Tools, gathered at run time.** The loop knows tools only by their protocol (`Tool`: name, description, conventions, start, observe, act, content, capture). The app registers them in a toolbox; with several, the model picks the one whose description fits the goal, and a BLOCKED tool hands the goal to another. The browser window opens only when a goal needs it.
- **Packages.** `agent` (contracts), `decision-loop` (the loop), `browser` (a tool), `openrouter` (a decision-model provider), `session` (a history store), `apps/cli` (the binary). Dependencies point inward; `apps/cli/tests/architecture.rs` checks it.
- **Renamed.** The binary is `decision-loop`; environment variables are `DECISION_LOOP_MODEL` (was `--decision-model` / `LXSURF_DECISION_MODEL`, now `--model`), `DECISION_LOOP_HEADLESS`, `DECISION_LOOP_SESSIONS`, `DECISION_LOOP_DEBUG` and `BROWSER_DEBUG`; the key lives in `~/.config/decision-loop/credentials`, sessions in `~/.decision-loop/sessions`.
- **`--json` schema 2.** `usage` has `decision` only; `tool`, `final_location` (was `final_url`) and `screenshot` are new; `data`, `data_error` and the `unstated` status are gone; an expectation has no `by`.
- **The page as an outline.** The browser shows the model its page as an outline: headings and text in reading order, every control with its ref, value and state, by structure (which question a radio answers, which result a price belongs to). `--dry-run` and `/snapshot` print it.
- **Wait, as long as needed.** WAIT carries a length the model picks (1 to 30 s), ending early when the view changes.
- **Screenshots.** `--screenshot FILE` saves a PNG of the final view, and `/screenshot` in the chat; the browser takes WebKit's own snapshot, in an invisible window too (macOS).
- **Chat and sessions.** `--keep-open` leaves a chat: each line is the next goal in the same window, or `/snapshot`, `/screenshot [FILE]`, `/tools`. Every run is a session (`decision-loop sessions`, `-c`, `--resume ID`). `--timeout` bounds each goal.
- **Decision models.** `decision-loop models` lists OpenRouter's decision models cheapest first, with price and context.
- **Steadier on real sites** (evals 42/42, median 5 s, $0.0007 a task):
  - a quoted string is typed exactly as written, and when the goal quotes values only those are offered;
  - a filled field and a placed value are not offered again;
  - consent banners in same-origin frames are seen and accepted; a click under one that loads late is decided again;
  - a script or a click that leaves the page no longer waits for a reply that cannot come, and a parsed document is used after at most 1 s more instead of waiting up to 20 s for every ad and video;
  - with the Navigation API reporting the page's own navigations, actions on ad-heavy pages no longer wait 300 ms for frames' navigations;
  - the page's own requests in flight mark it busy; an unsure DONE, or one while fetching, is decided again only if the view changes;
  - the step history is numbered, marks steps that opened a new view, and counts ("three questions") hold.
- A `<button>` is named by its text, not its `value`; a refused action shows why in the trace.

## 0.1.0

First release, as a browser CLI with a chat model that planned and a decision model that clicked and typed: `check` and `--expect`, `--json` and `--trace`, stable exit codes, headless by default under CI, OpenRouter sign-in, viewports, cost reports, key presses, page scripts, date pickers, guides for agents.
