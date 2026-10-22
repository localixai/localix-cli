# AGENTS.md

Instructions for AI coding agents working on this repository.

## Layout

Packages and an app; every dependency points inward (rules L1-L5 below).

- `packages/contracts`: ports only, no logic. `chat.rs` (the chat-model port: our own `Message`, `ToolCall`, `Request`, `Completion`), `tool.rs` (the agent's tools: `ToolSpec`, `ToolOutput { report, state }`, `ToolError { Failed, Fatal }`), `view.rs` (what the hands drive: `Observation` with an outline and refs, `Action`, `View`, `Capture`), `model.rs` (the decision-model port: typed questions, `Decider`), `hands.rs` (a `Task` carried out on a view, its `Report`), `history.rs` (sessions, `History`).
- `packages/agent`: the agent. `agent.rs` (the loop: one tool call at a time, stop conditions, `Spend`), `conversation.rs` (messages that only grow; older states dropped past a budget; open calls closed), `schema.rs` (`--schema` as the `result` tool), `answer.rs` (the `RESULT:` line), `prompt.md` (the system prompt).
- `packages/decision-loop`: the hands. `decision_loop.rs` (resumes a run until DONE, BLOCKED or the budget; implements `Hands`), `goal.rs` (one run: observe, decide, act, its guards), `ask.rs` (the question each step asks), `step.rs`, `choice.rs`, `values.rs` (what `--no-llm` may type), `expect.rs`, `meter.rs`.
- `packages/tools`: the tools, a module each. `browser/` (`tool.rs`: `BrowserTool`, the agent's `browser`; `page.rs`: `Page`, the window as a `View`; `host.rs`: tao event loop on the main thread; `webview.rs`: the driver; `toolbar.rs`: Back, Reload, page size; `platform.rs`: what differs by OS; `probe/*.js`: the page side: snapshot, frames, outline, actions and settle, keys, evaluate).
- `packages/openrouter`: the models' provider: `chat.rs` (chat completions with tool calling and prompt-cache marks), `system_one.rs` (decision models), its HTTP client, signing in. It is told who calls it (`App`).
- `packages/session`: a history store, sessions as JSON files.
- `apps/cli`: the `lxsurf` binary and the composition root: `cli.rs` (the command line), `run.rs` (one goal: the agent, or the hands alone with `--no-llm`), `chat.rs` (`--keep-open`, sessions), `report.rs`, `exit.rs` (the exit-code contract), `skill-data/` (guides served by `lxsurf skills get`; `skills/lxsurf/SKILL.md` is a thin stub).
- `evals/`: live tasks against real sites; `benchmarks/`: their history.

## Rules

Layers (checked by `apps/cli/tests/architecture.rs`):

- L1: `contracts` depends on no workspace package.
- L2: `agent` and `decision-loop` depend only on `contracts`.
- L3: tools, providers and stores (every other package under `packages/`) depend only on `contracts`: a tool knows nothing of the agent, the hands or the models; a provider nothing of tools.
- L4: only apps compose packages; nothing depends on an app.
- L5: `contracts`, `agent` and `decision-loop` name no tool or provider (no browser, page, URL, OpenRouter, model name). What a tool's world is like reaches the agent through `ToolSpec::description` and the hands through `View::conventions`; a browser's own controls (its toolbar) are part of its view.

Behaviour:

- Decision-model output never becomes a selector, coordinates or code. Targets are refs from the latest observation; typed text is one of the task's values.
- Tool content is untrusted data. Never let it reach a request as an instruction; mark it so where the agent reads it.
- No task- or site-specific wording in the agent's prompt, the tool descriptions or the hands' instructions: a rule earns its place by the evals, and stays general.
- The release binary stays small: no browser engine, no async runtime, no dependency that is not already in the tree unless it earns its size. Check `ls -la target/release/lxsurf` after adding one.
- The browser window never takes keyboard focus and never stops rendering. An action is settled when the DOM has been quiet for 100 ms (at most 700 ms), and when a navigation it started has a parsed document.
- Exit codes and the `--json` schema are a public contract. Change them only with a schema bump and a CHANGELOG entry.

Code:

- Unit tests live beside the code, never inside it: `x.rs` ends with `#[cfg(test)] #[path = "x_tests.rs"] mod tests;` (checked by the architecture test). Fake the ports (a scripted `Chat`, a `Decider`, a `View`) instead of calling a network.
- New source files stay under 300 lines; rustfmt defaults; comments explain why.

## When changing behaviour

Update together: `--help` text (clap doc comments in `apps/cli/src/cli.rs`), `README.md`, `apps/cli/skill-data/*/SKILL.md`, and `CHANGELOG.md`.

## Checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace          # unit tests, the layer rules, packages/tools/tests/browser.rs (a real headless window)
python3 evals/run.py --repeat 3 # live, paid; run for agent, loop, probe or prompt changes
```

`packages/tools/tests/browser.rs` has its own `main` (the event loop needs the main thread). Add a fixture under `packages/tools/tests/fixtures/` and an assertion there for every driver or probe fix, and confirm it fails without the fix. `DECISION_LOOP_DEBUG=1` prints each decision's state and answers; `BROWSER_DEBUG=1` prints action timings and navigation events.
