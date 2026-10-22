# llx

[![ci](https://github.com/rsaryev/llx/actions/workflows/ci.yml/badge.svg)](https://github.com/rsaryev/llx/actions/workflows/ci.yml)
[![release](https://img.shields.io/github/v/release/rsaryev/llx?include_prereleases&sort=semver)](https://github.com/rsaryev/llx/releases)

[llx](https://llx.sh) is an ultrafast, lightweight coding agent for the terminal, installed as `llx` and, shorter, `llx`. It has five tools:

- `read` and `edit` for files;
- `bash` for commands;
- a real browser. The agent says what to do on a page, and a cheap decision model does the clicking and typing;
- `ask`, a question to you, in a session, when a decision is yours.

It is one binary of about 1.6 MB, with synchronous code and no async runtime. There are no plugins, no MCP servers and no extra tools: a model with a shell and a browser needs little more.

```text
$ llx exec -y "the tests in tests/parse.rs fail: fix the parser, not the tests"
   1.1s ▸ bash cargo test --test parse
   3.4s ▸ read src/parse.rs
   5.0s ▸ edit src/parse.rs
   6.2s ▸ bash cargo test --test parse
achieved in 9.0s · 5 turns · $0.0058
  agent    anthropic/claude-haiku-5.5 · 23,410 in (17,920 cached) · 655 out · $0.0058 · 6.1s
  session  1792675200-3fa2
parse_range treated an empty upper bound as 0; it now means "to the end". All 14 tests pass.
```

The output above is an example: times, tokens and cost vary from run to run. The trace goes to stderr and the answer alone to stdout, so `llx exec "..." > answer.md` keeps only the answer.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/rsaryev/llx/main/scripts/install.sh | sh
```

- **macOS and Linux x86_64:** the script downloads the latest release and checks it against `SHA256SUMS`, then installs `llx` into `~/.local/bin`. To install a given version, set `LLX_VERSION=0.2.0`. To install elsewhere, set `LLX_INSTALL_DIR`.
- **Windows:** run `irm https://raw.githubusercontent.com/rsaryev/llx/main/scripts/install.ps1 | iex`.
- **From a clone:** run `cargo install --locked --path apps/cli` (the tested `Cargo.lock`). It needs Rust 1.88 or newer.

llx uses the system's own WebView for the browser:

- macOS has WKWebView built in.
- Windows needs WebView2, which comes with Windows 11.
- Linux needs WebKitGTK (`libwebkit2gtk-4.1-0`) and a display. Without a display, llx runs without the browser tool.

## Quick start

```bash
llx login        # sign in with OpenRouter in your browser; or export OPENROUTER_API_KEY=sk-or-...
llx doctor       # checks the key, both models, the WebView and the shell
cd your-project
llx              # a session: type a task, each one goes on from the last
```

## Usage

llx works the way Codex and Claude Code do: `llx` opens a session at the terminal, and `llx exec` (or `-p`) runs one task and exits.

```bash
llx                                                        # a session at the terminal
llx "fix the failing test in src/parse.rs"                 # a session that starts with this task
llx -c                                                     # go on with the newest session here
llx exec "fix the failing test in src/parse.rs"            # one task, then exit (also: llx e, llx -p)
llx exec -y "run the tests and fix what fails"             # allow commands and edits without asking
llx --mode plan "how should we split src/parse.rs?"        # investigate and plan; change no file
git diff | llx -p "review this change"                     # piped stdin is added to the prompt
llx exec - < task.md                                       # the prompt from stdin
llx exec -y "fix the lint" </dev/null                      # from a script whose stdin stays open
llx exec -C ../site "open http://localhost:3000 and check that sign-up works"
llx exec --json -y "update the changelog" | jq -c 'select(.type == "tool.call")'
llx exec --schema '{"type":"object","properties":{"version":{"type":"string"}},"required":["version"]}' \
    "which version is this crate?"                        # the data alone on stdout
llx exec resume --last "now update the README"             # one more task in the newest session
```

In a session, each line you type is the next task, in the same conversation. A line ending in `\` goes on on the next. Slash commands: `/mode [name]` (also `/permissions`) lists the [modes](#modes-and-approvals) or switches to one, `/plan [task]` enters plan mode (and runs the task in it), Shift+Tab then Enter toggles plan mode, `/new` (also `/clear`) starts a new conversation and session, `/status` shows the models, the workspace, the mode, the session and any terminal session still running, `/help`, and `/exit` (also `/quit`, or Ctrl-D). The prompt shows the mode when it is not `default` (`plan › `). A session needs a terminal on stdin; without one, use `llx exec`.

| Option | Meaning |
| --- | --- |
| `-C, --cd DIR` | The workspace: paths resolve in it and files change only inside it. Default: the current directory. |
| `--mode MODE` | `default`, `plan`, `bypass` or `autonomous` (see [Modes and approvals](#modes-and-approvals)). Also `--permission-mode`. Default: `default`. |
| `-y, --yes` | `--mode bypass`: allow commands, file changes and browser actions without asking. Also `--yolo` and `--dangerously-skip-permissions`. Giving it with `--mode` is a usage error. |
| `-p, --print` | Run the prompt as one task and exit, as `exec` does. |
| `-c, --continue` | Go on with the newest session of this directory, as `resume --last` does. |
| `--json` | With `exec` or `-p`: one JSON event per line on stdout (see [`--json`](#--json)). |
| `--schema SCHEMA` | With `exec` or `-p`: end with data matching this JSON Schema (inline, or `@file`), printed alone on stdout. |
| `-o, --output-last-message FILE` | With `exec` or `-p`: also write the final answer, or the data with `--schema`, to FILE. With `--schema` and no data that matched, the file is left empty. |
| `-v, --verbose` | Show each tool result and model turn in the trace. With `--json`, the trace also goes to stderr. |
| `-m, --model MODEL` | The agent's chat model, any OpenRouter model id. Env `LLX_MODEL`, default `anthropic/claude-haiku-5.5`. |
| `--decision-model MODEL` | The browser's decision model. Env `LLX_DECISION_MODEL`, default `~typesafe/jev-latest`. |
| `--max-turns N` | Model turns for one task (50; 1 to 10000). |
| `--max-steps N` | Actions for one browser task of the decision model (40; 1 to 10000). |
| `--timeout SECONDS` | Seconds one task may take (1800; at most 86400). It stops at the first model turn past them. Time spent waiting on you (an approval, a question) does not count. |
| `--show-browser` | Show the browser window. It is invisible by default, and opens only when the browser is first used. A shown window is an ordinary one: it comes to the front when it opens, without taking the keyboard, and goes behind other windows like any other. |

Commands:

- `login`: sign in with OpenRouter. `login --with-token` reads the key from stdin instead (at a terminal, with the echo off), and `login status` shows where the key comes from, masked.
- `logout`: forget the saved key.
- `sessions`: list the saved sessions, newest first, with the directory each ran in.
- `exec [PROMPT | -]` (alias `e`): run one task without a session and exit, with the answer alone on stdout. `exec resume [ID | --last] [PROMPT]` runs one more task in a saved session.
- `resume [ID | --last] [PROMPT]`: go on with a session at the terminal. `--last` is the newest session that ran in this workspace (else the newest of all, with a note).
- `doctor`: check that everything a run needs is in place.

Every run is saved as a session in `~/.lx/sessions/<id>.json`; set `LLX_SESSIONS` to keep them elsewhere. A session records each task and how it ended, but not the conversation itself. `resume` therefore starts a new conversation that is told the earlier tasks, the directory they ran in, and where the browser was. In a session, each task continues the same conversation.

With `exec`, given a PROMPT, stdin that is not a terminal is read to its end and added to the prompt, and llx says so on stderr. A caller whose stdin stays open (a harness with a pipe, `ssh` without `-n`, a `while read` loop) should pass `</dev/null`.

## Modes and approvals

`read` never asks. `edit`, `bash` and `browser` ask before they act, through one gate (`browser` when a call opens an address, hands over a goal, presses keys or runs a script; reading a page does not ask). The mode decides what the gate answers, and whether `ask` gets an answer:

| Mode | What happens |
| --- | --- |
| `default` | Each command, file change and browser action is asked at the terminal. With no terminal, they are refused, and the agent is told the person must rerun with `--yes` or `--mode bypass`. Reading still works. |
| `plan` | The agent investigates and answers with a plan: a title, a summary, the changes file by file, the tests, its assumptions. File changes are refused without asking. Each command, each start of a terminal session and each text typed into one, and each browser action is asked, so it can look but you decide what runs; plan keeps its own `a` answers, apart from the default mode's, and `a` for `bash` covers its terminal sessions as it covers its commands. With no terminal, commands and browser actions are refused too, and the plan comes from reading files. |
| `bypass` (`-y`) | Everything is allowed without asking. `ask` still reaches you. |
| `autonomous` | Everything is allowed without asking, and nobody answers questions: `ask` fails at once and the agent goes on with its best assumption. In `exec`, where `ask` is not offered, it is the same as `bypass`. |

The mode is `--mode` (or `-y`), else `default`, for a session and for `exec` alike. In a session, `/mode <name>` switches it for the next question, `/plan` enters plan mode, and a line that is only Shift+Tab (then Enter) toggles plan mode on and off: the prompt line is read as typed lines, so Shift+Tab reaches llx only with Enter. The agent is told of a change before its next task, in a `<mode_change>` note. When a task in plan mode ends well, llx asks `Implement this plan?`: `y` goes back to the mode plan mode was entered from and runs `Implement the plan.`; `n` or Enter stays in plan mode; anything else is your next entry, so you can revise the plan by typing what to change.

At a terminal, each effect is shown in full on one line (`bash: cargo test`, `bash: rm -rf build  (in ~/src, outside the workspace)`, `edit: edit src/lib.rs (+3 -1 lines)`); line breaks show as `⏎`, and control characters are escaped, so what you approve is what runs. A terminal session asks when it starts (`bash: python3 -q  (in a terminal)`) and before each text or key typed into it, showing exactly what will be typed (`bash: type into session 1 (`python3 -q`): print(1) ⏎ [Enter]`); reading its output and closing it do not ask. Answer `y` to allow it, `n` or Enter to refuse it, `n <why>` to refuse it and tell the agent why, or `a` to allow that tool for the rest of the run. Only the bare word approves: `yes, and...` is asked again. What was typed before the question appeared is discarded. The question goes to the terminal itself, so this works while stdin is a pipe, stdout carries JSON and stderr goes to a file.

Ctrl-C stops the task: the command running is killed with everything it started, and so is every terminal session `bash` keeps, an open approval or question ends at once, nothing more is approved, and the task ends at its next model turn as `interrupted` (exit 130), with its `run.completed` event and its session saved. A second Ctrl-C quits at once, killing the terminal sessions first. At the session's prompt, Ctrl-C quits. Terminal sessions also end when the task does (its answer, `--timeout`), on `/new`, and when llx exits. Closing the browser window stops the task the same way; in a session the window opens again on the next task.

A refusal goes back to the model as a failed call with its reason, and it is told not to work around it.

## Project instructions

llx reads `AGENTS.md` files the way Codex does: from the git repository's root down to the workspace, root first. Each one covers its own directory tree, a deeper file wins over a shallower one, and the task you give wins over both. Each file is capped at 16 KiB and all of them together at 32 KiB, with a note when something was cut. They reach the agent in a context block with the environment: the workspace, the OS, the date, the mode, and whether a browser is available (the shell is named in the `bash` tool's description).

## Tools

| Tool | What it does |
| --- | --- |
| `read` | A file's lines, numbered, up to 2000 lines and about 20 KB at a time (`offset`, `limit`), or a directory's entries. Long lines are cut, binary files and anything but regular files (a FIFO, a device) refused. It reads anywhere (a dependency's source, a system header) except llx's own key and sessions. |
| `edit` | Exact replacement: `old_string` must occur once, unless `replace_all` is set. An empty `old_string` creates a file. Writes are atomic (a new temporary file, then a rename) and keep CRLF line endings and permissions; a read-only file is refused, and a file with other hard links is written in place. It asks the gate, refuses to write when the file changed while the question was open, and changes only files inside the workspace. |
| `bash` | `sh -c` (`cmd /C` on Windows) in the workspace or a `workdir`, with stdin closed. On Unix it runs in its own session, with no terminal, so a program that prompts fails at once; git and pagers are told not to prompt or page. A timeout (120 s by default, at most 600 s) or Ctrl-C kills it with everything it started. `OPENROUTER_API_KEY` is left out of a command's environment; that is no barrier, since a command can still read llx's own environment (`ps -E`, `/proc/$PPID/environ`) and the saved key file. Each stream keeps its first and last 5,000 bytes. A non-zero exit is a normal result. It asks the gate. On Unix, a program that must ask (a REPL, an installer's questions, a login) runs with `tty` in a terminal of its own (120×40, `TERM=dumb`, `NO_COLOR=1`): the call returns once the output settles (`yield_ms`, 2 s by default) or `wait_for` text appears, with a session id while the program runs. Later calls type into it with `session`, `input` (at most 4096 bytes, and 1000 on one line: a terminal drops the rest of a longer line) and named `keys` (Enter, arrows, Ctrl-C, …), read its new output with `session` alone, or end it with `close`, which kills every process of its session, the jobs a shell put in groups of their own included (only a process that calls `setsid` itself escapes). Output is a transcript, not a screen: colours and cursor moves are dropped, and a progress bar keeps its last state. At most 4 sessions run at once, and every session ends with its task. |
| `ask` | One to three questions to you, in a session only (not in `exec`). Each is shown with its options numbered: answer with a number, a number and a note (`2, keep it local`), or your own words; a question that allows several answers takes several numbers (`1,3 and a note`); Enter skips it, and Ctrl-C stops the task. It asks no gate: it changes nothing. In `autonomous` mode it fails at once, so the agent decides on its own. |
| `browser` | A native WebView window. The agent opens an http(s) address, or hands a short task with the exact values to type to the decision model, which clicks, types, selects and scrolls one step at a time. It can also press keys, run a script in the page, read the whole text, change the page size, take a screenshot, or close the window (the next call opens a fresh one). Each call returns what happened and the page as an outline. A screenshot (`screenshot: true`, cropped to an element with a CSS `selector`) is the WebView's own picture, canvas and WebGL included, sent to the model as a JPEG of at most 1280 pixels on its long side, after the call's result; only the newest three stay in the conversation. Screenshots work on macOS; elsewhere the call says they are not supported. It asks the gate before anything a page sees; a screenshot and a close do not ask. |

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | The task was achieved, or the agent answered without saying it was not |
| 1 | The agent answered that the task was not achieved, or the data did not match `--schema` |
| 2 | Turns or the time limit ran out before an answer |
| 3 | The environment or the command line is not ready: no key, no network, a model error, a bad argument |
| 130 | Interrupted: Ctrl-C (or SIGTERM), or the browser window was closed |

With `--json`, a closed stdout (`llx --json ... | head -1`) ends the run quietly at its next turn, with exit 0. Without it nothing is written to stdout until the answer, so the run goes on to its end and then exits 0.

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
| `packages/contracts` | Ports only: the chat model, the tools, the gate, the person, the view the browser's hands drive, the decision model, the hands, the history | nothing |
| `packages/agent` | The agent: the chat model's loop over tools, the conversation, `--schema`, the system prompt | `contracts` |
| `packages/decision-loop` | The browser's hands: a short task carried out on a view, one decision-model request per step | `contracts` |
| `packages/tools` | `read`, `edit`, `bash`, `ask`, the workspace, and the browser (window, driver, page probe) | `contracts` |
| `packages/openrouter` | The models' provider (chat and decision models) and signing in | `contracts` |
| `packages/session` | Sessions as JSON files | `contracts` |
| `apps/cli` | The `llx` binary: the command line, the modes and approvals, who answers `ask`, the context, the output; it composes the packages | all of the above |

The agent knows its tools only by their specs, the hands know a view only through its protocol, and neither names a tool or a provider. A tool asks the gate before an effect, and the person a question, but only the app answers them.

## Trust boundaries

- **Tool output is data, not instructions.** File contents, command output and pages are untrusted. The system prompt says so, and every page reaches the agent marked as untrusted content. This lowers the risk of prompt injection without removing it: approve commands with that in mind, and use `-y` in places where a wrong command costs little.
- **The workspace limits `edit`, not `bash`.** `edit` changes only files inside the workspace, after following symlinks. `bash` is a real shell with your permissions, and there is no sandbox: the gate is what stands between the model and your machine. A `workdir` outside the workspace is named as such in the question.
- **llx's own files are off limits** to `read` and `edit`, symlinks included: the saved key and the sessions directory.
- **The browser's decision model only chooses** among the operations and controls a page offers, and types only the strings the agent gave it. Its output never becomes a selector, coordinates or code. Only http(s) pages load; downloads and other schemes are refused. A password field is typed into, but its value is never read back.
- **Keys:** the key `llx login` saves lives in `~/.config/llx/credentials` (under `XDG_CONFIG_HOME` when set, `%APPDATA%` on Windows), mode 0600 on Unix, also when an older file was wider. `llx logout` removes it. `OPENROUTER_API_KEY` wins over it when set. Commands start without the variable in their environment, but a command can still read the key from llx's environment or from the saved file: what keeps it from a command is approving each one (not `--yes`, `bypass` or `autonomous`).

Known gaps:

- Terminal sessions (`bash` with `tty`) have no screen: a full-screen program (an editor, `top`, a pager) shows only fragments of its text, so the agent is steered to batch modes. The window is fixed at 120×40. There are none on Windows: `tty` is not offered there. A session never outlives its task, so a long-lived server still runs as `cmd > log 2>&1 &`. `wait_for` can match the echo of the text typed; the report says so when the text was in the input.
- On Windows, Ctrl-C ends llx and the commands it started at once (they share the console), with no `run.completed` event, and `login --with-token` shows the key as it is pasted.

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

`python3 evals/run.py --record` runs the live evals, then appends a dated file to `benchmarks/runs/` and regenerates [benchmarks/HISTORY.md](benchmarks/HISTORY.md) and this block. The coding tasks write and run a script, fix a failing test, rename a function across files, answer questions about code, answer an installer's prompts in a terminal session and plan a feature in `--mode plan`, each on a small fixture. The browser tasks cover a login, forms, a TodoMVC app, delayed content, search, a quiz behind a consent banner and a Google Flights search. See [evals/README.md](evals/README.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md) (the layer rules and the checks). Report security issues as described in [SECURITY.md](SECURITY.md).

## License

MIT
