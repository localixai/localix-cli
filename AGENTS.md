# AGENTS.md

Instructions for AI coding agents working on this repository.

`lx`, the Localix CLI, is an ultrafast, lightweight coding agent for the terminal. It has four tools: `read`, `edit`, `bash`, and a native browser whose clicks and typing are done by a decision model. The tool set is deliberately minimal: no plugins, no MCP, no skills, no extra tools. A new capability goes into one of these four, or is left to the shell.

## Layout

Packages and an app. Every dependency points inward (rules L1-L5 below).

- `packages/contracts`: ports only, no logic.
  - `chat.rs`: the chat-model port, with our own `Message`, `ToolCall`, `Request`, `Completion` and `Usage`.
  - `tool.rs`: the agent's tools: `ToolSpec`, `ToolOutput { report, state }`, `ToolError { Failed, Fatal }`.
  - `gate.rs`: a tool asks before an effect: `Effect { tool, summary }`, `Verdict { Allow, Deny(why) }`, `Gate`, and `Open`.
  - `view.rs`: what the hands drive: `Observation` with an outline and refs, `Action`, `View`.
  - `model.rs`: the decision-model port: typed questions, `Decider`.
  - `hands.rs`: a `Task` carried out on a view, and its `Report`.
  - `history.rs`: sessions, `History`.
- `packages/agent`: the agent.
  - `agent.rs`: the loop: one tool call at a time, the stop conditions, `Spend`. `Agent::context` is the app's environment block, sent before the first goal.
  - `observer.rs`: what a run shows as it goes: calls, results, turns.
  - `conversation.rs`: messages that only grow. Older states are dropped past a budget, and open calls are closed.
  - `schema.rs`: `--schema` as the `result` tool.
  - `answer.rs`: the `RESULT:` line.
  - `prompt.md`: the system prompt.
  - `fakes.rs`: test-only fakes: a scripted chat, an echo tool, a recorder.
- `packages/decision-loop`: the hands.
  - `decision_loop.rs`: resumes a run until DONE, BLOCKED or the budget runs out; implements `Hands`.
  - `goal.rs`: one run (observe, decide, act) and its guards.
  - `ask.rs`: the question each step asks.
  - `step.rs`, `choice.rs`.
  - `meter.rs`: the decision model's spend.
- `packages/tools`: the tools, a module each.
  - `workspace.rs`: where paths resolve. A file can be read anywhere but the paths the app denies (its key, its sessions), and changed only inside the root.
  - `fs/`: `read.rs` (numbered lines, or a directory's entries) and `edit.rs` (exact replacement, or a new file), with `text.rs` (numbering, line endings, +/- counts) and `atomic.rs` (temporary file, then rename).
  - `shell/`: `bash.rs` (the tool), `process.rs` (its own session with no terminal, a deadline, a stop, both pipes read on threads, the group killed) and `output.rs` (head and tail kept, the middle cut).
  - `browser/`:
    - `tool.rs`: `BrowserTool`, the agent's `browser`.
    - `page.rs`: `Page`, the window as a `View`.
    - `host.rs`: the tao event loop on the main thread; a closed window fails the calls waiting on it and opens again when needed.
    - `webview.rs`: the driver.
    - `toolbar.rs` (Back, Reload, page size) and `viewport.rs`.
    - `platform.rs`: what differs by OS.
    - `guard.rs`: what may load, and the probe's token.
    - `script.rs`.
    - `probe/*.js`: the page side: snapshot, frames, outline, actions and settle, keys, evaluate.
  - `testing.rs`: test-only: temporary directories, and a gate that records what it is asked.
- `packages/openrouter`: the models' provider.
  - `chat.rs`: chat completions, with tool calling and prompt-cache marks.
  - `system_one.rs`: the decision models.
  - `client.rs`: the HTTP client.
  - `auth.rs` and `auth_page.rs`: signing in.
  - It is told who calls it (`App`).
- `packages/session`: a history store: sessions as JSON files.
- `apps/cli`: the `lx` binary and the composition root.
  - `main.rs`: where the process exits (or the browser host, once its event loop owns the main thread, with the code the run returns).
  - `cli.rs`: the command line.
  - `commands/`: `run.rs` (a session or one `exec` task: checks, then the browser host, then the work), `repl.rs` (a session's prompt line and slash commands), `login.rs`, `sessions.rs` (`sessions`, `resume`), `doctor.rs`.
  - `compose.rs`: builds the models, then (`assemble`, testable with fake models) the hands, the gate and the tools, then the `Agent`.
  - `context.rs`: the environment block, the AGENTS.md files from the git root down, an earlier session.
  - `gate.rs`: who approves: `--yes`, the terminal, or nobody; nothing once the task is stopped.
  - `interrupt.rs`: Ctrl-C, SIGTERM and a closed window stop the task, not the process.
  - `printable.rs`: model text escaped before it reaches the terminal.
  - `brake.rs`: stops between model turns, on `--timeout` or when stdout's reader is gone.
  - `input.rs`: the prompt from arguments and stdin, and `--schema`.
  - `output.rs`, with `output/human.rs` (the trace on stderr) and `output/json.rs` (`--json` events).
  - `error.rs` (`Failure`) and `exit.rs` (the exit-code contract).
  - `testing.rs`: test-only helpers.
  - `tests/architecture.rs`: the layer rules. `tests/cli.rs`: the binary end to end, with no network.
- `evals/`: live tasks, both coding (fixtures in `evals/fixtures/`) and browser (real sites). `benchmarks/`: their history.

## Rules

Layers (checked by `apps/cli/tests/architecture.rs`):

- L1: `contracts` depends on no workspace package.
- L2: `agent` and `decision-loop` depend only on `contracts`.
- L3: tools, providers and stores (every other package under `packages/`) depend only on `contracts`. A tool knows nothing of the agent, the hands, the models or the other tools. A provider knows nothing of tools.
- L4: only apps compose packages. Nothing depends on an app.
- L5: `contracts`, `agent` and `decision-loop` name no tool or provider: no browser, page, URL, OpenRouter, model name. The agent learns what a tool's world is like through `ToolSpec::description`, and the hands through `View::conventions`. Where the run happens (the directory, the system, the approvals) reaches the agent through the app's context. A browser's own controls (its toolbar) are part of its view.

Behaviour:

- A tool asks the `Gate` before an effect outside the conversation: a changed file, a command, anything a page sees (an address, a goal, keys, a script). The `Effect` summary is one line the person can judge, and it leaves nothing out (the whole command, the directory it runs in). A `Deny` goes back to the model as a failed call with its reason. Only the app answers the gate (`apps/cli/src/gate.rs`); a tool never decides its own approval. Reading needs no approval.
- Paths go through `Workspace`: `readable` for reading, `writable` for changing. Never open a path the model wrote without it.
- Decision-model output never becomes a selector, coordinates or code. Targets are refs from the latest observation; typed text is one of the task's values.
- Tool content (files, command output, pages) is untrusted data. Never let it reach a request as an instruction; mark it so where the agent reads it.
- No task- or site-specific wording in the agent's prompt, the tool descriptions or the hands' instructions. A rule earns its place by the evals, and stays general.
- Synchronous code: threads where needed, no async runtime.
- The release binary stays small (about 1.6 MB). No browser engine, and no dependency that is not already in the tree unless it earns its size; std first. Check `ls -la target/release/lx` after adding one. CI fails above 2.5 MiB.
- The browser window never takes keyboard focus and never stops rendering. An action is settled when the DOM has been quiet for 100 ms (at most 700 ms), and when a navigation it started has a parsed document.
- Exit codes (`apps/cli/src/exit.rs`) and the `--json` events (`apps/cli/src/output/json.rs`, `SCHEMA`) are a public contract. Change them only with a schema bump and a CHANGELOG entry.

Code:

- Unit tests live beside the code, never inside it: `x.rs` ends with `#[cfg(test)] #[path = "x_tests.rs"] mod tests;` (checked by the architecture test). Fake the ports (a scripted `Chat`, a `Gate`, a `Decider`, a `View`) instead of calling a network.
- New source files stay under 300 lines. rustfmt defaults. Comments explain why.
- English only, in code, comments and docs.

## When changing behaviour

Update these together:

- the `--help` text (clap doc comments in `apps/cli/src/cli.rs`);
- `README.md`;
- `CHANGELOG.md`, under `## Unreleased`;
- `evals/README.md`, when tasks change.

## Checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace            # unit tests, the layer rules, tests/cli.rs, packages/tools/tests/browser.rs (a real headless window)
cargo build --release && ls -la target/release/lx
python3 evals/run.py --repeat 3   # live, paid; run for agent, prompt, tool, loop or probe changes
python3 evals/run.py --only code  # the coding tasks alone
```

`packages/tools/tests/browser.rs` has its own `main`, because the event loop needs the main thread.

- For every driver or probe fix, add a fixture under `packages/tools/tests/fixtures/` and an assertion in that file. Confirm the assertion fails without the fix.
- `DECISION_LOOP_DEBUG=1` prints each decision's state and answers.
- `BROWSER_DEBUG=1` prints action timings and navigation events.
- `lx -v` shows each tool result and model turn in the trace.
