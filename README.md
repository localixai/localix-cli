# llx

[![ci](https://github.com/rsaryev/llx/actions/workflows/ci.yml/badge.svg)](https://github.com/rsaryev/llx/actions/workflows/ci.yml)
[![release](https://img.shields.io/github/v/release/rsaryev/llx?include_prereleases&sort=semver)](https://github.com/rsaryev/llx/releases)

[llx](https://llx.sh) is an ultrafast, lightweight coding agent for the terminal, installed as `llx`. It has seven tools:

- `read` and `edit` for files;
- `bash` for commands;
- `ask`, a question to you, in a session, when a decision is yours;
- `agents`, other agents it starts in parallel for self-contained parts of the work: each one a session of its own, which you can switch to;
- `jobs`: monitors (a background command whose lines come back as they are printed) and schedules (a prompt that comes back at a time);
- `window`: everything the agent opens to look at and work in, as tabs of one window: real web pages and terminal sessions, and more kinds later. It opens several at once and reads each one's snapshot as text, acts in a tab (with a short `goal` that a cheap decision model carries out on any kind of tab, or the kind's own fields), takes a screenshot, and shows you the tab you ask for.

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
- **Windows** is not supported for now.
- **From a clone:** run `cargo install --locked --path apps/cli` (the tested `Cargo.lock`). It needs Rust 1.88 or newer.

llx uses the system's own WebView for the browser:

- macOS has WKWebView built in.
- Linux needs WebKitGTK (`libwebkit2gtk-4.1-0`) and a display. Without a display, llx runs without the browser tool.

## Quick start

```bash
cd your-project
llx              # a session: type /login first, then a task; each one goes on from the last
llx doctor       # checks the sign-ins, both models, the WebView and the shell
```

`/login` asks for a provider (every one of the catalog in one list: the popular first, then the current one and those signed in to, then the rest by name; `✓` marks those signed in to or with a key set; type to search it by name or id), signs you in to it, then opens its model list (↑↓ choose the model, ←/→ change its reasoning level, Enter confirms):

- **OpenAI**, with a ChatGPT sign-in: your browser opens OpenAI's sign-in page and comes back to llx on `127.0.0.1:1455` (or 1457), and llx goes on by itself, with nothing to paste; requests are billed to your ChatGPT plan, and `/login` and `/model` offer the models the ChatGPT backend serves, those it serves alone (the Codex and Pro models) too. On a machine without a browser, open the address shown elsewhere and paste the address the browser ends on.
- **OpenRouter**, with a key: paste it (it is not shown), or sign in in the browser for a new one. `OPENROUTER_API_KEY` works too, and wins over a saved key.
- **Any provider of the model catalog** (Anthropic, Google, xAI, DeepSeek, Mistral, Groq, Moonshot, Z.ai, Together, Fireworks… about 200), with a key: paste it, or set the variable models.dev names for it (`ANTHROPIC_API_KEY`, `DEEPSEEK_API_KEY`…), which wins over a saved key. OpenAI takes an API key too. Each is called over OpenAI's chat completions at the address the catalog gives.

The models, their context windows, reasoning levels and defaults come from one catalog, models.dev's ([MODELS.md](MODELS.md)): its files for the popular providers are built in, and a session fetches its newest data in the background, at most once a day, for the next one. Each request's cost comes from the catalog's prices when the provider reports none (a ChatGPT plan's as its API-equivalent value). `/model` chooses among the models of every provider signed in to; ←/→ change the highlighted model's reasoning level inline, and Enter confirms both without a second menu. The current model keeps its level, and other models start at their own defaults; a list longer than 12 is searched by typing (Backspace edits the query, Esc clears it, then cancels) and scrolls with ↑↓, PageUp, PageDown, Home and End; Shift+Up and Shift+Down move the level without a menu.

The choice is put in at once, in the same conversation, and saved as the default for later sessions and for `exec`. The browser tool's decision model runs on OpenRouter, so browser actions need an OpenRouter key whichever provider the agent thinks with.

## Usage

`llx` opens a session at the terminal, and `llx exec` (or `-p`) runs one task and exits.

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
llx exec resume --last now update the README               # one more task in the newest session
```

In a session, what you type is the next task, in the same conversation. While a task works, Enter sends what you type to it, read at its next step (`↳`); Tab keeps it for after the task (`⏎`). Under the prompt, a list shows the session, the agents it started and the jobs running (other sessions are one line); ↓ from an empty prompt gives it the keys: ↑↓ choose, Enter or → shows an agent's transcript, `x` stops an agent or a job, Esc or ← go back. `/model` chooses the model and its reasoning level. The session is drawn into the terminal: what finished goes into the terminal's own scrollback (so scrolling, selecting and copying are the terminal's), and only the bottom is redrawn: the call running, `Working (12s · esc to interrupt)`, the messages you queued meanwhile, and the prompt `❯`. Enter sends; Shift+Enter, Ctrl-J or a line ending in `\` start a new line, and a paste keeps its lines; Up and Down walk earlier entries. Typing `/` lists commands; Tab completes the selected command (with a trailing space for `/mode`, `/permissions`, and `/plan`), while Enter runs exactly what is typed: `/mode` lists modes, `/plan` enters plan mode, and `/mode plan` switches modes. Mode names are suggested after `/mode ` and `/permissions `; Tab completes a name, Enter applies the typed name. Clipboard text is shown in the composer as `[Paste]` (numbered for multiple pastes) and expanded to its original text when sent; pasted file paths still become file or image marks. `/login` signs in and chooses the provider, the model and its reasoning level (see [Quick start](#quick-start)), `/mode [name]` (also `/permissions`) lists the [modes](#modes-and-approvals) or switches to one, `/plan [task]` enters plan mode (and runs the task in it), `/new` (also `/clear`) opens a new session and shows it, even while a task works: the old one goes on in the background with its agents, jobs and terminal sessions (each session is recorded in its own file, and Ctrl-C stops only the one shown; an approval or a question from one not shown names it, `session <name> (in the background)`, ignores keys for 300 ms and is answered as any other, and its own choices wait until it is shown). ← on an empty prompt lists the sessions in this project: those open (working, then waiting for you, then idle or done), "+ New session", then this directory's saved sessions, newest first; ↑↓ choose, Enter or → opens one (a saved one is resumed in a session of its own), `x` stops a session's task without switching to it, Esc or ← close the list. At most 8 are open: another closes the oldest idle one. When a session not shown finishes, the one shown says `● Session "…" finished · ← to open`, `/status` shows the models, the workspace, the mode, the session and any terminal session still running, `/rewind` (also `/undo`, or Esc Esc on an empty prompt) goes back to before one of the conversation's tasks: the files `edit` changed (the agents' too; not what commands changed), the conversation (the task's words back in the prompt; the session's agents are let go), or both; `/diff` shows the changes since the last commit, new files too; `/init` writes an AGENTS.md for the project; `/copy` copies the last answer that says something; `/usage` (also `/cost`) shows what the session spent (the footer's right end says it all along: `24.1k in · 87% cached · 1.2k out · $0.042`, a resumed session's earlier tasks included); Ctrl-R finds the last earlier entry with the words already typed (type them first; again for older ones) and Ctrl-G writes the entry in `$EDITOR`; `/help`, and `/exit` (also `/quit`, or Ctrl-D). Shift+Tab cycles default → plan → bypass → autonomous → default without Enter; while a task runs, the switch waits until it ends. Files go in: drag a file into the terminal or paste its path, and it becomes `[image: shot.png]` for a picture (PNG, JPEG, GIF, WebP, up to 8 MB, sent to the model as one, from a copy kept in `~/.llx/pasted`) or `[file: notes.md]` for anything else (sent as its path, for the agent to read); several at once work too. `@` followed by a few letters lists the workspace's files, and Enter or Tab puts the chosen path in. Ctrl+V attaches the clipboard's picture (macOS). `-i FILE` (`--image`) attaches pictures to the first task, in a session or in `exec`; a file that is missing or not a picture is a usage error before any model is called. Shift+Tab cycles default → plan → bypass → autonomous → default without Enter; while a task runs, the switch waits until it ends. A message typed while a task runs waits for it to end. A session needs a terminal on stdin that its interface can draw on; without one (or where the interface cannot start), it is a usage error pointing to `llx exec`, which prints the same blocks (each call, the answer, how it ended) without the live part.

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
| `--provider PROVIDER` | Where the agent's chat model comes from: a provider of the model catalog by id: `openai` (a ChatGPT sign-in or a key), `openrouter`, `anthropic`, `deepseek`, `xai`… Env `LLX_PROVIDER`. Default: the one `/login` saved, else the one signed in to. |
| `-m, --model MODEL` | The agent's chat model, a model id of the provider's (any OpenRouter id works). Env `LLX_MODEL`. Default: the one `/login` saved, else `gpt-6-luna` on OpenAI and `anthropic/claude-haiku-5.5` on OpenRouter. |
| `--reasoning LEVEL` | How much the chat model reasons: `off`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max` or `ultra`, as far as the model takes it (a level it does not take is a usage error; `/login` lists its levels). Env `LLX_REASONING`. Default: the one `/login` saved for that model, else the model's own. |
| `--decision-model MODEL` | The browser's decision model. Env `LLX_DECISION_MODEL`, default `~typesafe/jev-latest`. |
| `--max-turns N` | Model turns for one task (50; 1 to 10000). |
| `--max-steps N` | Actions for one browser task of the decision model (40; 1 to 10000). |
| `--timeout SECONDS` | Seconds one task may take (1800; at most 86400). It stops at the first model turn past them, or while the task waits on its monitors or agents. Time spent waiting on you (an approval, a question) does not count. |
| `--no-daemon` | Run the session in this process instead of the daemon (`llx serve`). Env `LLX_NO_DAEMON=1`. `exec` always runs in process. |
| `--show-browser` | Show the browser window, and a window for each terminal session `bash` keeps. By default they show in a session, and stay hidden in `exec`, in autonomous mode and over SSH (`SSH_CONNECTION` or `SSH_TTY` set; the browser then works without a window). The browser window opens only when the browser is first used. A shown window is an ordinary one: it comes to the front when it opens, without taking the keyboard, and goes behind other windows like any other. |

Commands:

- `logout`: forget every saved sign-in: the ChatGPT tokens and the OpenRouter key. (Signing in is `/login` in a session.)
- `sessions`: list the saved sessions, newest first, with the directory each ran in.
- `exec [PROMPT | -]` (alias `e`): run one task without a session and exit, with the answer alone on stdout. `exec resume [ID | --last] [PROMPT]` runs one more task in a saved session.
- `resume [ID | --last] [PROMPT]`: go on with a session at the terminal. `--last` is the newest session that ran in this workspace (else the newest of all, with a note).
- `doctor`: check that everything a run needs is in place.
- `serve`: the daemon the sessions run in (unix). `serve status` says whether it runs (its pid, its build, the terminals attached, the sessions open and working, those moved to the background, the jobs; exit 1 when none runs), `serve stop` ends it, stopping every session's task first.

A session at the terminal runs in a daemon, `llx serve`: one per config directory, found at a socket in `$TMPDIR/llx-<uid>/` (a directory only you may enter), started by the first `llx` (detached, in a session of its own, its errors in `daemon.log` beside the sign-ins) and attached to by every later one. The daemon holds the sessions with their agents, jobs and terminal sessions, the models, and the one window; `llx` draws, reads the keys and speaks to it over the socket. Each terminal's sessions work in its directory, with its environment, and with the options its command line gave; each terminal has its own questions, and quitting one names only its own work. A session is open at one terminal at a time: resuming one open at another says so. Closing a terminal stops its sessions' work; the daemon itself lives on until `llx serve stop`. Closing the window there stops nothing: the tasks working in it are told, and it opens again when next used. A plain `llx` in a directory where sessions were moved to the background comes back to them; `llx` with a task, `resume` or any option opens new sessions beside them, which wait for the next plain `llx`. A daemon of another build is stopped and replaced only when nothing needs it (no terminal attached, no session open or moved to the background, no task, monitor or schedule); otherwise the session runs in this process, and llx says so, as it does when the daemon cannot be reached. `--no-daemon` (or `LLX_NO_DAEMON=1`) always runs in process, and so does `exec`.

Successful `edit` calls show an inline diff with old/new line numbers, surrounding context, red background bands for removals and green bands for additions. The bands fill the row and its continuations, including in terminal scrollback. Long code lines wrap instead of being clipped. Large previews are bounded and explicitly marked when truncated; the diff is shown after the edit succeeds.

Markdown answers support tables with aligned columns, wrapped cells and inline styles. On very narrow terminals, table cells are shown vertically so their contents remain readable.

Under the prompt, the tree shows only the session on the screen and the agents it started (as a branch), and the jobs; with no agents it is gone. The other sessions take one line, `← 2 other sessions (1 working, 1 waiting for you)`, and ← lists them. Time and tokens follow each row's status. Each session gets a short title from its first task, made in the background by the same model at its lowest reasoning level (the task never waits for it), shown in the tree and the ← list and saved with the session. ↓ from an empty prompt focuses it; ↑↓ select, →/Enter open a transcript, Esc returns to input, and ← returns from an agent to its session. Consecutive successful calls of the same tool are joined with `↳` instead of separated by blank rows; failures and questions remain distinct.

Every run is saved as a session in `~/.llx/sessions/<id>.json`; set `LLX_SESSIONS` to keep them elsewhere. A session records each task and how it ended, but not the conversation itself. `resume` therefore starts a new conversation that is told the earlier tasks, the directory they ran in, and where the browser was; at the terminal, the earlier tasks and their answers are drawn first (the last 20), so you see where the session left off. In a session, each task continues the same conversation.

With `exec`, given a PROMPT, stdin that is not a terminal is read to its end and added to the prompt, and llx says so on stderr. A caller whose stdin stays open (a harness with a pipe, `ssh` without `-n`, a `while read` loop) should pass `</dev/null`.

## Modes and approvals

`read` never asks. `edit`, `bash` and `browser` ask before they act, through one gate (`browser` when a call opens an address, hands over a goal, presses keys or runs a script; reading a page does not ask). The mode decides what the gate answers, and whether `ask` gets an answer:

| Mode | What happens |
| --- | --- |
| `default` | Each command, file change and browser action is asked at the terminal. With no terminal, they are refused, and the agent is told the person must rerun with `--yes` or `--mode bypass`. Reading still works. |
| `plan` | The agent investigates and answers with a plan: a title, a summary, the changes file by file, the tests, its assumptions. File changes are refused without asking. Each command, each start of a terminal session and each text typed into one, and each browser action is asked, so it can look but you decide what runs; plan keeps its own `a` answers, apart from the default mode's, and `a` for `bash` covers its terminal sessions as it covers its commands. With no terminal, commands and browser actions are refused too, and the plan comes from reading files. |
| `bypass` (`-y`) | Everything is allowed without asking. `ask` still reaches you. |
| `autonomous` | Everything is allowed without asking, and nobody answers questions: `ask` fails at once and the agent goes on with its best assumption. In `exec`, where `ask` is not offered, it is the same as `bypass`. |

The mode is `--mode` (or `-y`), else `default`, for a session and for `exec` alike. In a session, `/mode <name>` switches it for the next question, `/plan` enters plan mode, and Shift+Tab cycles default → plan → bypass → autonomous → default without Enter (queued until the current task ends). The current mode and `shift+tab to cycle` are always shown under the prompt, including in default mode and while typing. The agent is told of a change before its next task, in a `<mode_change>` note. When a task in plan mode ends well, llx asks `Implement this plan?`: yes goes back to the mode plan mode was entered from and runs `Implement the plan.`; no stays in plan mode, where your next message can revise the plan.

In a session, an approval replaces the prompt with what would happen (`Run this command?`, then the command in full) and three choices: `1. Yes`, `2. Yes, and don't ask again for bash`, `3. No, and tell llx what to do differently`, which opens a line for why. Pick with the arrows and Enter, or a number; Esc says no. Letters pick nothing, so words typed as the box opens cannot approve or refuse. Line breaks show as `⏎` and control characters are escaped, so what you approve is what runs. A terminal session asks when it starts and before each text or key typed into it, showing exactly what will be typed; reading its output and closing it do not ask. Outside a session (`exec` with a terminal), the question goes to the terminal itself as one line: `y`, `n`, `n <why>` or `a`, where only the bare word approves; this works while stdin is a pipe, stdout carries JSON and stderr goes to a file.

Esc or Ctrl-C stops the task: the command running is killed with everything it started (and the terminal session a call is waiting on; the others stay open), an open approval or question ends at once, nothing more is approved, and the task ends at its next model turn as `interrupted` (exit 130 in `exec`), with its session saved. A second Ctrl-C quits at once, killing the terminal sessions first. Ctrl-C closes what is open first (a question says no; the list under the prompt or the ← list closes, and nothing more happens). While a task works, it stops the task and keeps what you were typing. With text typed and no task, it clears the text into the earlier entries (↑ brings it back) and says `↑ brings it back · ctrl-c again to quit`; at an empty prompt it says `ctrl-c again to quit`; a second Ctrl-C within 2 s quits, any other key cancels. Ctrl-D at an empty prompt (or `/exit`) quits at once, even while a task works. Esc on an empty prompt says `esc again to rewind`. While a session works or a shell, a monitor or a schedule runs, quitting asks first (`Background work is running`, then each thing that would stop: `session · <its title>`, `shell · <its command>`, `monitor · <name>`, `schedule · <name>`): "Exit and stop tasks" stops every session, waits at most 2 s for them and exits (a task whose model request is still in flight then is not recorded); "Move to background and exit", offered only when the sessions run in the daemon, leaves them working there, and the next `llx` in that directory comes back to them: what they did meanwhile is drawn (the transcript, then any question still waiting), and you go on in the same conversation; "Stay" (or Esc) closes the question. A terminal closed without that choice stops the work, as "Exit and stop tasks" does. SIGTERM or closing the window stops every session's task (in the daemon, closing the window stops none). Terminal sessions end when llx exits, and in `exec` when the task does (its answer, `--timeout`). Closing the browser window stops the task the same way; in a session the window opens again on the next task.

A refusal goes back to the model as a failed call with its reason, and it is told not to work around it.

## Project instructions

llx reads instruction files, so a repository that already has them works as it is:

- your own, for every project: `~/.llx/AGENTS.md`;
- the project's: in each directory from the git repository's root down to the workspace, the first of `AGENTS.override.md`, `AGENTS.md` and `CLAUDE.md`, root first, so a deeper file wins over a shallower one;
- a directory below the workspace: its file comes with the result of the first `read` or `edit` of a file there, and covers that directory's tree.

A line that is only `@path` brings that file in (relative to the file, or `~/…`), three levels deep, each file once, as in CLAUDE.md. The task you give wins over every file. Each file is capped at 16 KiB and those read at the start at 32 KiB together, with a note when something was cut. They reach the agent in a context block with the environment: the workspace, the OS, the date, the mode, and whether a browser is available.

The context is built so the prompt cache keeps working. It is a message of its own after the system prompt, the same for every task and session in a workspace on a day (the date has no time), and it is marked for the cache, so a new session reads it from there. Files are read once per session; a subdirectory's file is appended with a call's result; a mode change is a note appended later; old page states and screenshots are dropped together once past a budget, not one at a time. Nothing earlier in the conversation is rewritten turn by turn.

## Tools

| Tool | What it does |
| --- | --- |
| `read` | A file's lines, numbered, up to 2000 lines and about 20 KB at a time (`offset`, `limit`), or a directory's entries. Long lines are cut, binary files and anything but regular files (a FIFO, a device) refused. It reads anywhere (a dependency's source, a system header) except llx's own key and sessions. |
| `edit` | Exact replacement: `old_string` must occur once, unless `replace_all` is set. An empty `old_string` creates a file. Writes are atomic (a new temporary file, then a rename) and keep CRLF line endings and permissions; a read-only file is refused, and a file with other hard links is written in place. It asks the gate, refuses to write when the file changed while the question was open, and changes only files inside the workspace. |
| `bash` | `sh -c` in the workspace or a `workdir`, with stdin closed. On Unix it runs in its own session, with no terminal, so a program that prompts fails at once; git and pagers are told not to prompt or page. A timeout (120 s by default, at most 600 s) or Ctrl-C kills it with everything it started. The key variables of the provider in use (as the catalog names them, such as `DEEPSEEK_API_KEY`) and `OPENROUTER_API_KEY` are left out of a command's environment; other providers' variables stay, so a project's own tests can use them; a provider chosen later with `/login` has its variables left out too, from the next command on. That is no barrier, since a command can still read llx's own environment (`ps -E`, `/proc/$PPID/environ`) and the saved key file. Each stream keeps its first and last 5,000 bytes. A non-zero exit is a normal result. It asks the gate. A program that must ask (a REPL, an installer, another agent's interface) runs in a terminal tab of `window`. |
| `agents` | Starts other agents and works with them, in parallel: `spawn` (a task, and a short name) starts one on a thread of its own and returns at once, `send` tells one more (read at its next step while it works, or a new task once it is done), `wait` (some or all, `timeout_ms`, 30 s by default) returns the answers that came, `list` shows them, `close` ends one (at most 6 open). An agent's answer, its last message, reaches the main agent by itself at its next step as an `<agent_result>` message; a task does not end while its agents work. An agent is a session of its own whose parent is the main one: its conversation opens with the same context, it has the same tools but `agents` and `ask` (one level, and nobody to answer it), and asks the same gate, its approvals marked `[agent name]` and asked one at a time. It sees nothing of the main conversation. It stops with the task (Ctrl-C, the time limit) or alone (`x` on its row), and what it spends counts in the task's turns and cost (and in `--json`'s `turn` events and `usage`). Its tasks are saved as a session that names its parent; `llx sessions` lists the main ones with how many agents each started, and `resume` never picks an agent's. |
| `jobs` | Work that comes back later. `monitor` runs a command in the background (approved like a command, in its own process group, without the key variables `bash` leaves out): each batch of lines it prints reaches the main agent as a `<monitor>` message, at its next step while a task runs, else as a task of its own; it ends when the command exits, at its deadline (5 min by default, 30 at most; 10 in `exec`, where the task waits for it) or with `stop`, and the agent is told. `schedule` gives a `prompt` back as a task of its own at a time: `in` (`45m`, `2h`) or a 5-field `cron` in local time (`*/10 * * * *`, `0 9 * * 1-5`; both day fields restricted match either), once or `recurring` (for 7 days at most); it runs between tasks, behind what you queued, while the session is open (not in `exec`; not restored by `resume`; no jitter). At most 8 monitors and 50 jobs. `list`, `stop`. |
| `window` | Everything the agent opens, as tabs of one window, each kind a strategy behind one interface: `open` (one tab, or several at once with `tabs`: `{"kind": "browser", "url": …}`, `{"kind": "terminal", "command": …}`) returns each tab's id (`w1`, `t2`) and its snapshot, the text it shows (a page's outline; a terminal's transcript, or its screen when the program draws over its output); `interact` acts in a tab, with a short `goal` and the exact `values` it may type, carried out by the decision model on any kind of tab through its view (a page's controls; a terminal's input line and keys), or with the kind's own fields (a page's `keys`, `script`, `read`; a terminal's `input`, `keys`, `wait_for`), and returns the snapshot after; `screenshot` pictures a tab; `close` ends one or several (`tabs`); where you watch the window, `list` names every tab, `show` puts one in front of you and returns its snapshot, `resize` sizes the window. A goal, an address, keys, a script, starting a program and typing into it are approved first. Terminal sessions run on between tasks, at most 4. |
| `ask` | One to three questions to you, in a session only (not in `exec`). Each is shown with its options to pick with the arrows or a number (a question that allows several answers toggles them with Space, then Enter), and a last one to type your own answer; Esc skips it, and Ctrl-C stops the task. It asks no gate: it changes nothing. In `autonomous` mode it fails at once, so the agent decides on its own. |
| `window` browser tabs | A native WebView. The agent opens an http(s) address (one with no scheme opens over http on this machine, `localhost`, `127.*`, `0.0.0.0` or `[::1]`, and over https elsewhere), or hands a short task with the exact values to type to the decision model, which clicks, types, selects and scrolls one step at a time. It can also press keys, run a script in the page, read the whole text, change the page size, take a screenshot, or close the window (the next call opens a fresh one). It keeps up to 8 tabs in the one window: `tab: "new"` opens an address in a new tab, `tab: 2` switches to tab 2, `close_tab: 2` closes it, and a link with `target=_blank` or a `window.open` opens a tab instead of being lost. Every other part of a call acts on the tab shown; the list of tabs comes back only when it changed. Tabs out of sight keep running and drawing (they stand beside the window, not hidden), and on macOS they share one cookie store, as in a browser. Each call returns what happened and the page as an outline. A screenshot (`screenshot: true`, cropped to an element with a CSS `selector`) is the WebView's own picture, canvas and WebGL included, sent to the model as a JPEG of at most 1280 pixels on its long side, after the call's result; only the newest three stay in the conversation. Screenshots work on macOS; elsewhere the call says they are not supported. It asks the gate before anything a page sees; a screenshot and a close do not ask. |

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

The workspace is split into packages and one app. Every dependency points inward, and `apps/cli/tests/suite/repo/architecture.rs` checks that:

| Package | What it is | Depends on |
| --- | --- | --- |
| `packages/contracts` | Ports only: the chat model, the tools, the gate, the person, the view the browser's hands drive, the decision model, the hands, the history | nothing |
| `packages/agent` | The agent: the chat model's loop over tools, the conversation, `--schema`, the system prompt | `contracts` |
| `packages/decision-loop` | The browser's hands: a short task carried out on a view, one decision-model request per step | `contracts` |
| `packages/tools` | `read`, `edit`, `bash`, `ask`, the workspace, and the browser (window, driver, page probe) | `contracts` |
| `packages/providers/kit` | What the providers share: an HTTP client on the OS's TLS, PKCE, the sign-in callback server and its pages, private credential files, server-sent events | nothing |
| `packages/providers/openrouter` | OpenRouter: its chat models (with a catalog of models and their reasoning levels), the decision model, its key and sign-in | `contracts`, the kit |
| `packages/providers/openai` | OpenAI with a ChatGPT sign-in: the OAuth flow and token refresh, the Responses API of the ChatGPT backend, the models a plan reaches | `contracts`, the kit |
| `packages/session` | Sessions as JSON files | `contracts` |
| `apps/cli` | The `llx` binary: the command line, the modes and approvals, who answers `ask`, the context, the output; it composes the packages | all of the above |

The agent knows its tools only by their specs, the hands know a view only through its protocol, and neither names a tool or a provider. A provider implements the chat port (and the decision port where it has decision models) and lists its models with the reasoning levels each takes; only `apps/cli/src/providers.rs` knows the provider packages, so a new provider is a package under `packages/providers/` and one arm per `match` there. A tool asks the gate before an effect, and the person a question, but only the app answers them.

## Trust boundaries

- **Tool output is data, not instructions.** File contents, command output and pages are untrusted. The system prompt says so, and every page reaches the agent marked as untrusted content. This lowers the risk of prompt injection without removing it: approve commands with that in mind, and use `-y` in places where a wrong command costs little.
- **The workspace limits `edit`, not `bash`.** `edit` changes only files inside the workspace, after following symlinks. `bash` is a real shell with your permissions, and there is no sandbox: the gate is what stands between the model and your machine. A `workdir` outside the workspace is named as such in the question.
- **llx's own files are off limits** to `read` and `edit`, symlinks included: its config directory (the saved key, the ChatGPT tokens, the `/login` choice) and the sessions directory.
- **The browser's decision model only chooses** among the operations and controls a page offers, and types only the strings the agent gave it. Its output never becomes a selector, coordinates or code. Only http(s) pages load; downloads and other schemes are refused. A password field is typed into, but its value is never read back.
- **Keys and tokens:** `/login` saves an OpenRouter key in `~/.config/llx/credentials` and a ChatGPT sign-in's tokens (access, refresh, ID, and the account id) in `~/.config/llx/openai.json` (under `XDG_CONFIG_HOME` when set), mode 0600, also when an older file was wider; the choice of provider, model and level goes in `settings.json` beside them. Keys and tokens are never printed or logged; a pasted key is read with the echo off and shown only as its masked ends. The ChatGPT access token is refreshed five minutes before it lapses (and once when a request is refused), and the refreshed tokens are saved. `llx logout` removes both files. `OPENROUTER_API_KEY` wins over a saved key when set. Commands start without it, or the key variables of the provider in use (or of one chosen later with `/login`), in their environment, but a command can still read the key from llx's environment or from the saved file: what keeps it from a command is approving each one (not `--yes`, `bypass` or `autonomous`).

Known gaps:

- Terminal sessions (`bash` with `tty`) have no screen for the model (the window is for you): a full-screen program (an editor, `top`, a pager) shows only fragments of its text, so the agent is steered to batch modes. The window is fixed at 120×40. A session never outlives its task, so a long-lived server still runs as `cmd > log 2>&1 &`. `wait_for` can match the echo of the text typed; the report says so when the text was in the input.

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
