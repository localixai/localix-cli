# lxsurf

[![ci](https://github.com/rsaryev/decision-loop/actions/workflows/ci.yml/badge.svg)](https://github.com/rsaryev/decision-loop/actions/workflows/ci.yml)
[![release](https://img.shields.io/github/v/release/rsaryev/decision-loop?include_prereleases&sort=semver)](https://github.com/rsaryev/decision-loop/releases)

lxsurf is a small, fast coding agent that runs in your terminal. It has four tools:

- `read` and `edit` for files;
- `bash` for commands;
- a real browser. The agent says what to do on a page, and a cheap decision model does the clicking and typing.

It is one binary of about 1.6 MB, with synchronous code and no async runtime. There are no plugins, no MCP servers and no extra tools: a model with a shell and a browser needs little more.

```text
$ lxsurf -y "the tests in tests/parse.rs fail: fix the parser, not the tests"
   1.1s ▸ bash cargo test --test parse
   3.4s ▸ read src/parse.rs
   5.0s ▸ edit src/parse.rs
   6.2s ▸ bash cargo test --test parse
achieved in 9.0s · 5 turns · $0.0058
  agent    anthropic/claude-haiku-5.5 · 23,410 in (17,920 cached) · 655 out · $0.0058 · 6.1s
  session  1792675200-3fa2
parse_range treated an empty upper bound as 0; it now means "to the end". All 14 tests pass.
```

The output above is an example: times, tokens and cost vary from run to run. The trace goes to stderr and the answer alone to stdout, so `lxsurf "..." > answer.md` keeps only the answer.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/rsaryev/decision-loop/master/scripts/install.sh | sh
```

- **macOS and Linux x86_64:** the script downloads the latest release and checks it against `SHA256SUMS`, then installs `lxsurf` into `~/.local/bin`. To install a given version, set `LXSURF_VERSION=0.2.0`. To install elsewhere, set `LXSURF_INSTALL_DIR`.
- **Windows:** run `irm https://raw.githubusercontent.com/rsaryev/decision-loop/master/scripts/install.ps1 | iex`.
- **From a clone:** run `cargo install --locked --path apps/cli` (the tested `Cargo.lock`). It needs Rust 1.88 or newer.

lxsurf uses the system's own WebView for the browser:

- macOS has WKWebView built in.
- Windows needs WebView2, which comes with Windows 11.
- Linux needs WebKitGTK (`libwebkit2gtk-4.1-0`) and a display. Without a display, lxsurf runs without the browser tool.

## Quick start

```bash
lxsurf login        # sign in with OpenRouter in your browser; or export OPENROUTER_API_KEY=sk-or-...
lxsurf doctor       # checks the key, both models, the WebView and the shell
cd your-project
lxsurf "explain how errors are handled in this crate"
```

## Usage

```bash
lxsurf "fix the failing test in src/parse.rs"                 # one task, then exit
lxsurf -y "run the tests and fix what fails"                  # allow commands and edits without asking
lxsurf                                                        # the prompt line: each line is the next task
git diff | lxsurf "review this change"                        # piped stdin is added to the prompt
lxsurf - < task.md                                            # the prompt from stdin
lxsurf -y "fix the lint" </dev/null                           # from a script whose stdin stays open
lxsurf -C ../site "open http://localhost:3000 and check that sign-up works"
lxsurf --json -y "update the changelog" | jq -c 'select(.type == "tool.call")'
lxsurf --schema '{"type":"object","properties":{"version":{"type":"string"}},"required":["version"]}' \
    "which version is this crate?"                            # the data alone on stdout
lxsurf resume --last "now update the README"                  # go on with the newest session
```

| Option | Meaning |
| --- | --- |
| `-C, --cd DIR` | The workspace: paths resolve in it and files change only inside it. Default: the current directory. |
| `-y, --yes` | Allow commands, file changes and browser actions without asking. |
| `--json` | One JSON event per line on stdout (see [`--json`](#--json)). |
| `--schema SCHEMA` | End with data matching this JSON Schema (inline, or `@file`), printed alone on stdout. |
| `-o, --output-last-message FILE` | Also write the final answer, or the data with `--schema`, to FILE. With `--schema` and no data that matched, the file is left empty. |
| `-v, --verbose` | Show each tool result and model turn in the trace. With `--json`, the trace also goes to stderr. |
| `-m, --model MODEL` | The agent's chat model, any OpenRouter model id. Env `LXSURF_MODEL`, default `anthropic/claude-haiku-5.5`. |
| `--decision-model MODEL` | The browser's decision model. Env `LXSURF_DECISION_MODEL`, default `~typesafe/jev-latest`. |
| `--max-turns N` | Model turns for one task (50; 1 to 10000). |
| `--max-steps N` | Actions for one browser task of the decision model (40; 1 to 10000). |
| `--timeout SECONDS` | Seconds one task may take (1800; at most 86400). It stops at the first model turn past them. |
| `--show-browser` | Show the browser window. It is invisible by default, and opens only when the browser is first used. |

Commands:

- `login`: sign in with OpenRouter. `login --with-token` reads the key from stdin instead (at a terminal, with the echo off), and `login status` shows where the key comes from, masked.
- `logout`: forget the saved key.
- `sessions`: list the saved sessions, newest first, with the directory each ran in.
- `resume [ID | --last] [PROMPT]`: continue a session. `--last` is the newest session that ran in this workspace (else the newest of all, with a note).
- `doctor`: check that everything a run needs is in place.

Every run is saved as a session in `~/.lxsurf/sessions/<id>.json`; set `LXSURF_SESSIONS` to keep them elsewhere. A session records each task and how it ended, but not the conversation itself. `resume` therefore starts a new conversation that is told the earlier tasks, the directory they ran in, and where the browser was. In the prompt line, each task continues the same conversation.

Given a PROMPT, stdin that is not a terminal is read to its end and added to the prompt, and lxsurf says so on stderr. A caller whose stdin stays open (a harness with a pipe, `ssh` without `-n`, a `while read` loop) should pass `</dev/null`.

## Approvals

`read` never asks. `edit`, `bash` and `browser` ask before they act, through one gate (`browser` when a call opens an address, hands over a goal, presses keys or runs a script; reading a page does not ask). The answer depends on how lxsurf was started:

| Started | What happens |
| --- | --- |
| At a terminal | Each effect is shown in full on one line (`bash: cargo test`, `bash: rm -rf build  (in ~/src, outside the workspace)`, `edit: edit src/lib.rs (+3 -1 lines)`); line breaks show as `⏎`, and control characters are escaped, so what you approve is what runs. Answer `y` to allow it, `n` or Enter to refuse it, `n <why>` to refuse it and tell the agent why, or `a` to allow that tool for the rest of the run. Only the bare word approves: `yes, and...` is asked again. What was typed before the question appeared is discarded. The question goes to the terminal itself, so this works while stdin is a pipe, stdout carries JSON and stderr goes to a file. |
| With `-y` / `--yes` | Everything is allowed without asking. |
| With no terminal and no `-y` | Commands, edits and browser actions are refused, and the agent is told the person must rerun with `--yes`. Reading still works. |

Ctrl-C stops the task: the command running is killed with everything it started, nothing more is approved, and the task ends at its next model turn as `interrupted` (exit 130), with its `run.completed` event and its session saved. A second Ctrl-C quits at once. At the prompt line, Ctrl-C quits. Closing the browser window stops the task the same way; in the prompt line the window opens again on the next task.

A refusal goes back to the model as a failed call with its reason, and it is told not to work around it.

## Project instructions

lxsurf reads `AGENTS.md` files the way Codex does: from the git repository's root down to the workspace, root first. Each one covers its own directory tree, a deeper file wins over a shallower one, and the task you give wins over both. Each file is capped at 16 KiB and all of them together at 32 KiB, with a note when something was cut. They reach the agent in a context block with the environment: the workspace, the OS, the date, the approval policy, and whether a browser is available (the shell is named in the `bash` tool's description).

## Tools

| Tool | What it does |
| --- | --- |
| `read` | A file's lines, numbered, up to 2000 lines and about 20 KB at a time (`offset`, `limit`), or a directory's entries. Long lines are cut, binary files and anything but regular files (a FIFO, a device) refused. It reads anywhere (a dependency's source, a system header) except lxsurf's own key and sessions. |
| `edit` | Exact replacement: `old_string` must occur once, unless `replace_all` is set. An empty `old_string` creates a file. Writes are atomic (a new temporary file, then a rename) and keep CRLF line endings and permissions; a read-only file is refused, and a file with other hard links is written in place. It asks the gate, refuses to write when the file changed while the question was open, and changes only files inside the workspace. |
| `bash` | `sh -c` (`cmd /C` on Windows) in the workspace or a `workdir`, with stdin closed. On Unix it runs in its own session, with no terminal, so a program that prompts fails at once; git and pagers are told not to prompt or page. A timeout (120 s by default, at most 600 s) or Ctrl-C kills it with everything it started. Commands do not see `OPENROUTER_API_KEY`. Each stream keeps its first and last 5,000 bytes. A non-zero exit is a normal result. It asks the gate. |
| `browser` | A native WebView window. The agent opens an http(s) address, or hands a short task with the exact values to type to the decision model, which clicks, types, selects and scrolls one step at a time. It can also press keys, run a script in the page, read the whole text, or change the page size. Each call returns what happened and the page as an outline. It asks the gate before anything a page sees. |

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | The task was achieved, or the agent answered without saying it was not |
| 1 | The agent answered that the task was not achieved, or the data did not match `--schema` |
| 2 | Turns or the time limit ran out before an answer |
| 3 | The environment or the command line is not ready: no key, no network, a model error, a bad argument |
| 130 | Interrupted: Ctrl-C (or SIGTERM), or the browser window was closed |

With `--json`, a closed stdout (`lxsurf --json ... | head -1`) ends the run quietly at its next turn, with exit 0. Without it nothing is written to stdout until the answer, so the run goes on to its end and then exits 0.

## `--json`

`--json` prints one event per line on stdout, in the order things happen. Every event has a `type`, and `ms` is the time since the task started.

| Event | Fields |
| --- | --- |
| `run.started` | `schema` (4), `goal`, `session`, `model`, `decision_model`, `workspace` |
| `tool.call` | `id`, `name`, `arguments` |
| `tool.result` | `id`, `name`, `failed`, `fatal`, `output`, and `state` when the tool left one |
| `step` | One browser action: `operation`, `target`, `value`, `probability`, `changed`, `failed`, `line` |
| `turn` | One model turn: `model`, `input_tokens`, `cached_tokens`, `output_tokens`, `cost_usd`, `model_ms` |
| `run.completed` | `status` (`achieved`, `not_achieved`, `unstated`, `exhausted`, `error`, `interrupted`), `exit_code`, `answer`, `data` or `data_error`, `error`, `session`, `usage` {`agent`, `decision`, `cost_usd`}, `duration_ms` |

The exit codes and these events are a public contract. A breaking change bumps `schema` and is listed in [CHANGELOG.md](CHANGELOG.md).

## Architecture

The workspace is split into packages and one app. Every dependency points inward, and `apps/cli/tests/architecture.rs` checks that:

| Package | What it is | Depends on |
| --- | --- | --- |
| `packages/contracts` | Ports only: the chat model, the tools, the gate, the view the browser's hands drive, the decision model, the hands, the history | nothing |
| `packages/agent` | The agent: the chat model's loop over tools, the conversation, `--schema`, the system prompt | `contracts` |
| `packages/decision-loop` | The browser's hands: a short task carried out on a view, one decision-model request per step | `contracts` |
| `packages/tools` | `read`, `edit`, `bash`, the workspace, and the browser (window, driver, page probe) | `contracts` |
| `packages/openrouter` | The models' provider (chat and decision models) and signing in | `contracts` |
| `packages/session` | Sessions as JSON files | `contracts` |
| `apps/cli` | The `lxsurf` binary: the command line, the approvals, the context, the output; it composes the packages | all of the above |

The agent knows its tools only by their specs, the hands know a view only through its protocol, and neither names a tool or a provider. A tool asks the gate before an effect, but only the app answers it.

## Trust boundaries

- **Tool output is data, not instructions.** File contents, command output and pages are untrusted. The system prompt says so, and every page reaches the agent marked as untrusted content. This lowers the risk of prompt injection without removing it: approve commands with that in mind, and use `-y` in places where a wrong command costs little.
- **The workspace limits `edit`, not `bash`.** `edit` changes only files inside the workspace, after following symlinks. `bash` is a real shell with your permissions, and there is no sandbox: the gate is what stands between the model and your machine. A `workdir` outside the workspace is named as such in the question.
- **lxsurf's own files are off limits** to `read` and `edit`, symlinks included: the saved key and the sessions directory.
- **The browser's decision model only chooses** among the operations and controls a page offers, and types only the strings the agent gave it. Its output never becomes a selector, coordinates or code. Only http(s) pages load; downloads and other schemes are refused. A password field is typed into, but its value is never read back.
- **Keys:** the key `lxsurf login` saves lives in `~/.config/lxsurf/credentials` (under `XDG_CONFIG_HOME` when set, `%APPDATA%` on Windows), mode 0600 on Unix, also when an older file was wider. `lxsurf logout` removes it. `OPENROUTER_API_KEY` wins over it when set, and commands never see it.

Known gaps:

- On Windows, Ctrl-C ends lxsurf and the commands it started at once (they share the console), with no `run.completed` event, and `login --with-token` shows the key as it is pasted.

## Metrics

<!-- metrics:start -->
Latest run, 2026-10-22 22:17 on Darwin 25.6.0 arm64: the agent on anthropic/claude-haiku-5.5, the browser's hands on ~typesafe/jev-latest, from [benchmarks/HISTORY.md](benchmarks/HISTORY.md):

| Metric | Value |
| --- | --- |
| Eval runs passed | 24/24 (browser 16/16, code 8/8) |
| Task time, median / p90 | 8.5 s / 21.2 s |
| Cost per task | $0.0020 |
| Start (`--version`) | 3.8 ms |
| Memory at start (`--help`) | 9.6 MB |
| Release binary | 1.59 MB |
<!-- metrics:end -->

`python3 evals/run.py --record` runs the live evals, then appends a dated file to `benchmarks/runs/` and regenerates [benchmarks/HISTORY.md](benchmarks/HISTORY.md) and this block. The coding tasks write and run a script, fix a failing test, rename a function across files and answer questions about code, each on a small fixture. The browser tasks cover a login, forms, a TodoMVC app, delayed content, search, a quiz behind a consent banner and a Google Flights search. See [evals/README.md](evals/README.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md) (the layer rules and the checks). Report security issues as described in [SECURITY.md](SECURITY.md).

## License

MIT
