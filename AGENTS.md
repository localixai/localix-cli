# AGENTS.md

Instructions for AI coding agents working on this repository.

## Layout

- `src/browser/`: the native window (`host.rs`: tao event loop on the main thread, wry WebView; `platform.rs`: what differs by OS; `guard.rs`: allowed schemes and the run token; `viewport.rs`: page sizes; `script.rs`: how page scripts start), the worker-side driver (`webview.rs`), and the page probe (`probe/*.js`, concatenated into one script: snapshot, actions and settle, evaluate and the IPC entry).
- `src/step.rs` (operations, steps, stop reasons), `src/goal_ask.rs` (pure) and `src/goal.rs`: one sub-goal driven by the decision model. Ported from Localix's `browser-goal-ask.ts` and `browser-goal.ts`; keep their rules (offered operations, dialog scope, validate the answer, stop conditions) unless an eval shows a better one.
- `src/orchestrator.rs`, `src/tools.rs`, `src/planner.rs`, `src/prompt.md`: the planner loop with its one tool, `browser`.
- `src/decision.rs`, `src/openrouter.rs`, `src/auth.rs`: OpenRouter for both models; key from env or `lxsurf login`. `src/meter.rs` counts every call; `src/structured.rs` is `--schema` (the `result` tool and its validator). Keep every planner request with the same tools and system prompt, and the history append-only (compaction only past the budget): the prompt cache depends on it.
- `src/run.rs`: one CLI run (expectations, exit code); `src/report.rs`: trace, answer, JSON; `src/start.rs`: start page and visibility defaults. `src/exit.rs` is the exit-code contract.
- `skill-data/`: the agent guides served by `lxsurf skills get`. `skills/lxsurf/SKILL.md` is a thin discovery stub; do not put feature content there.
- `evals/`: live tasks against real sites.
- `examples/inspect.rs`: open a page and print scripts' results, for studying a site's DOM before changing the snapshot; `examples/auth_page.rs` renders the sign-in pages.

## Rules

- The release binary stays small: no browser engine, no async runtime, no dependency that is not already in the tree unless it earns its size. Check `ls -la target/release/lxsurf` after adding one.
- Model output never becomes a selector, coordinates or code. Targets are refs from the latest snapshot; typed text is one of the planner's `values`.
- Page content is untrusted data. Never let it reach a prompt as an instruction.
- The window never takes keyboard focus and never stops rendering: it floats, macOS occlusion detection is off, a headless window is transparent and click-through. Waits are timer-polled, not animation-frame driven.
- An action is settled when the DOM has been quiet for 100 ms (at most 700 ms), and when a navigation it started has loaded. The page reports a leaving document (`beforeunload`, Navigation API) before the native side hears of the load.
- Exit codes and the `--json` schema are a public contract. Change them only with a schema bump and a CHANGELOG entry.
- New source files stay under 300 lines; rustfmt defaults; comments explain why.

## When changing behaviour

Update together: `--help` text (clap doc comments), `README.md`, `skill-data/*/SKILL.md`, and `CHANGELOG.md`.

## Checks

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test                      # unit tests + tests/driver.rs (a real headless window, local fixtures)
OPENROUTER_API_KEY=... python3 evals/run.py --repeat 2   # live, paid; run for loop or prompt changes
```

`tests/driver.rs` has its own `main` (the event loop needs the main thread). Add a fixture under `tests/fixtures/` and an assertion there for every driver or probe fix, and confirm it fails without the fix. `LXSURF_DEBUG=1` prints decision states, action timings and navigation events.
