# AGENTS.md

Instructions for AI coding agents working on this repository.

## Layout

Packages and an app; every dependency points inward (rules L1-L5 below).

- `packages/agent`: the contracts. `tool.rs` (the tool protocol: `Observation` with an outline and refs, `Action`, `Tool`, `Capture`), `model.rs` (the decision-model port: typed questions, `Decider`), `history.rs` (sessions, `History`). No logic, no technology.
- `packages/decision-loop`: the loop. `decision_loop.rs` (picks a tool from the `Toolbox`, starts it, resumes runs, hands a BLOCKED goal on), `goal.rs` (one run: observe, decide, act, its guards), `ask.rs` (the question each step asks: operation and target heads, the wait length), `step.rs`, `choice.rs`, `values.rs` (quoted strings from the goal), `expect.rs`, `meter.rs`, `toolbox.rs`.
- `packages/browser`: a tool. `host.rs` (tao event loop on the main thread, the window opened on demand), `webview.rs` (the driver), `tool.rs` (`BrowserTool`: the protocol, conventions of web pages), `platform.rs` (what differs by OS, the macOS snapshot), `probe/*.js` (the page side: snapshot, same-origin frames, outline, actions and settle, keys, evaluate).
- `packages/openrouter`: a decision-model provider (`system_one.rs`), its HTTP client, and signing in (`auth.rs`, `auth_page.rs`). It is told who calls it (`App`).
- `packages/session`: a history store, sessions as JSON files.
- `apps/cli`: the `decision-loop` binary and the composition root: builds the provider, registers the tools in a `Toolbox`, keeps sessions, the chat (`chat.rs`), reports (`report.rs`), `exit.rs` (the exit-code contract), `skill-data/` (guides served by `decision-loop skills get`; `skills/decision-loop/SKILL.md` is a thin stub).
- `evals/`: live tasks against real sites; `benchmarks/`: their history.

## Rules

Layers (checked by `apps/cli/tests/architecture.rs`):

- L1: `agent` depends on no workspace package.
- L2: `decision-loop` depends only on `agent`.
- L3: tools, providers and stores (every other package under `packages/`) depend only on `agent`: a tool knows nothing of the loop or the model, a provider nothing of tools.
- L4: only apps compose packages; nothing depends on an app.
- L5: `agent` and `decision-loop` name no tool or provider (no browser, page, URL, OpenRouter, model name). What a tool's world is like comes through `Tool::description` and `Tool::conventions`.

Behaviour:

- Model output never becomes a selector, coordinates or code. Targets are refs from the latest observation; typed text is a string from the goal.
- Tool content is untrusted data. Never let it reach a request as an instruction.
- No task-specific wording in the loop's instructions: a rule earns its place by the evals, and stays general.
- The release binary stays small: no browser engine, no async runtime, no dependency that is not already in the tree unless it earns its size. Check `ls -la target/release/decision-loop` after adding one.
- The browser window never takes keyboard focus and never stops rendering. An action is settled when the DOM has been quiet for 100 ms (at most 700 ms), and when a navigation it started has a parsed document.
- Exit codes and the `--json` schema are a public contract. Change them only with a schema bump and a CHANGELOG entry.
- New source files stay under 300 lines; rustfmt defaults; comments explain why.

## When changing behaviour

Update together: `--help` text (clap doc comments), `README.md`, `apps/cli/skill-data/*/SKILL.md`, and `CHANGELOG.md`.

## Checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace          # unit tests, the layer rules, packages/browser/tests/driver.rs (a real headless window)
python3 evals/run.py --repeat 3 # live, paid (about $0.03); run for loop, probe or instruction changes
```

`packages/browser/tests/driver.rs` has its own `main` (the event loop needs the main thread). Add a fixture under `packages/browser/tests/fixtures/` and an assertion there for every driver or probe fix, and confirm it fails without the fix. `DECISION_LOOP_DEBUG=1` prints each decision's state and answers; `BROWSER_DEBUG=1` prints action timings and navigation events.
