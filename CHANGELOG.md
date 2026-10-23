# Changelog

## Unreleased

The browser CLI of 0.1.0 is now `llx`: an ultrafast, lightweight coding agent for the terminal. It has seven tools: `read`, `edit`, `bash`, the browser, whose clicks and typing a decision model does, `ask`, a question to the person in a session, `agents`, other agents it starts, and `jobs`, monitors and schedules. It stays one synchronous binary of about 2 MB, with no async runtime and no plugins, MCP or skills.

### Agents, parallel calls, steering, background jobs

- **`agents`: the agent starts other agents, in parallel.** One tool with an `action`: `spawn` (a self-contained task and a short name) starts an agent on a thread of its own and returns at once; `send` tells one more (read at its next step while it works, or a new task in its own conversation once it is done); `wait` (some agents, or all, with `timeout_ms`, 30 s by default) returns the answers that came; `list` and `close` (at most 6 open). An agent's answer, its last message (nothing is summarized), reaches the main agent by itself at its next step as an `<agent_result>` message, and a task does not end while its agents work. An agent is a session of its own whose parent is the main session (`parent` and `name` in its JSON file): its conversation opens with the same context, it works with tools of its own but neither `agents` (one level) nor `ask` (nobody answers an agent), behind the same gate, its approvals marked `[agent name]` and asked one at a time. It stops with the task, or alone with `x` on its row. What agents spend counts in the task's turns and cost, and their turns appear as `turn` events in `--json`. `llx sessions` lists the main sessions with how many agents each started; `resume` never picks an agent's session.
- **Independent calls run at once.** The model may make several calls in one turn (`parallel_tool_calls` is on for both providers), and the prompt asks it to: `read` and `bash` calls run together, the others alone and in order, and the results go back in the model's order. Reading five files is one turn, not five.
- **Steering.** A message sent while a task works goes to that task at its next step (shown `↳` until it is read); Tab keeps it for after the task instead (`⏎`), and so do commands and pictures. What the task did not get to read becomes the next task.
- **`jobs`: monitors and schedules.** `monitor` runs a command in the background (approved like a command), and each batch of lines it prints reaches the main agent as a `<monitor>` message: at its next step while a task runs, else as a task of its own; it ends when the command exits or at its deadline (5 min by default, 30 at most, 10 in `exec`, where the task waits for it), with a notice. `schedule` gives a prompt back as a task of its own at a time (`in: "45m"`, or a 5-field `cron` in local time, `recurring` for at most 7 days), between tasks and behind what the person queued (`◷`); at most 50 jobs; not in `exec`. `list` and `stop`.
- **One list for everything that runs.** Under the prompt: the main session, each agent (a spinner and its call, or how it ended; time and tokens on the right) and each job (`≋` a monitor with its events and time left, `◷` a schedule with its next run). ↓ from an empty prompt gives it the keys: ↑↓ choose, Enter or → shows an agent's transcript (the screen is drawn again from its start), `x` stops an agent or a job, Esc or ← give the keys back. The main transcript says when an agent finishes (`● Agent "scout" finished · 8s`) and while the task waits for agents.
- **`/model`** chooses the model and its reasoning level among the providers signed in to, without signing in again; `/login` stays for signing in.
- **Sessions side by side.** `/new` opens a new session and shows it, at once, even while a task works; the old one goes on in the background with its agents, jobs and terminal sessions. Each session runs on a thread of its own and is recorded in its own file; Ctrl-C stops only the session shown. Questions wait their turn, one open at a time (`1 more question waiting`); one from a session not shown names it (`session <name> (in the background)`) and ignores keys for 300 ms, and that session's own choices (carry out the plan?, `/login`'s steps) wait until it is shown; the name goes once it is.
- **Sessions in this project.** ← on an empty prompt lists them: the sessions open (working, then waiting for you, then idle or done, each with its time, tokens, tasks and agents), "+ New session", then this directory's saved sessions, newest first (at most 20, agents' hidden). ↑↓ choose, Enter or → opens (a session open is shown, its transcript drawn again; a saved one is resumed in a session of its own; a session is never open twice), `x` stops a session's task without switching to it, Esc or ← close the list. At most 8 sessions are open: another closes the oldest idle one, which the list can open again. When a session not shown finishes, the one shown says so (`● Session "…" finished · 3m 08s · ← to open`) and its row reads `unseen`. Under the prompt each session has a row, with its agents under it, and its time and tokens (its agents' included) on the right.
- **Quitting asks while work runs.** Ctrl-C twice, Ctrl-D and `/exit` ask `Quit llx?` when a session works or a shell or a monitor runs (`2 sessions still working · 3 shells and 1 monitor running — they will be stopped.`): "Quit anyway" stops every session, waits at most 2 s for them, gives the terminal back and exits; "Stay" (or Esc) closes it. `/exit` no longer waits for the tasks running. A second Ctrl-C while the session shown is stopping still quits at once when nothing else works.
- **`/rewind` warns** when another session is working in the directory: restoring may undo its changes.
- **No plain prompt line.** A session runs in its interface only: where the interface cannot start (stdin a terminal it cannot draw on), it is a usage error (exit 3) pointing to `llx exec`, as having no terminal is. `exec` is unchanged.
- The size budget (`apps/cli/tests/suite/repo/budget.rs`) is 25,000 lines of working code (build scripts count), raised by the owner.

### Models, context and the small commands

- **One model catalog: a Rust port of models.dev** (`packages/providers/models`, MODELS.md). models.dev's own files for the popular providers are built in and resolved as models.dev resolves them (lab models with each host's overrides, `base_model_omit`, every model's API addresses), checked against models.dev's published v2 resolution model by model (268 of 268 match); a session fetches the v2 API in the background, at most once a day with its ETag, and the next one reads it (225 providers, about 6,800 models). Every model fact comes from it: windows, reasoning levels (effort lists, toggles, budgets) and defaults, prices, addresses. The hand-written model tables are gone.
- **About 200 providers.** `--provider` takes any provider of the catalog by id, and `/login` lists them all: the popular first, then the current one and those signed in to, then the rest by name, those signed in to (or with a key set) marked `✓`. A key comes from the variable models.dev names for it, else a pasted one (`<config>/key-<id>`, 0600); OpenAI takes a ChatGPT sign-in or an API key. Every provider but the ChatGPT backend is called over OpenAI's chat completions at the address the catalog gives, through OpenRouter's client (`reasoning_effort` for any host but OpenRouter). `logout` lists only what it removed.
- **Each model's context limit, and the history fitted to it before every request** (`agent/src/history.rs`), without a summary: the limit is the window less a reserve, at most 272k; past 90% of it, old tool outputs (those a later call on the same path superseded first) and old calls' large arguments are replaced by pointers down to 60%, in one pass; the recent tail is never touched, and what was elided stays elided, so the prompt cache keeps its prefix. A strategy (`Strategy`, `Pipeline` of `Stage`s) the app chooses, so the way history is prepared can change; a test runs a 300-turn dialogue through it (24 cache breaks, the limit never passed). "N% context left" shows under the prompt once below half.
- **Every request is priced**: from the catalog's prices (uncached input, cached input at the cache-read price, output; the long-context band by the request's input), unless the provider reports a billed amount (OpenRouter does). A ChatGPT plan's requests show their API-equivalent value, so the cost line is never $0 for a model the catalog prices.
- **`/rewind`** (also `/undo`, Esc Esc on an empty prompt): back to before a task of the conversation, its files (what `edit` changed, the agents' too), the conversation (its words back in the prompt), or both.
- **Long lists are searched by typing.** A choice of more than 12 options (`/login`'s providers, `/model`'s models) shows `Search: open▏  2 matches`: letters filter it by name and id, what starts with the query first, then what holds it, then what has its letters as word starts; Backspace edits the query, Esc clears it and then cancels, Enter takes the marked row, and `No provider matches 'xyz'` says when nothing does. It scrolls with ↑↓, PageUp, PageDown, Home and End. A short list (an approval, a short choice) is numbered and picks by number, as before. The "Another provider…" entry, typed by id, is gone.
- **Shift+Up/Down** move the reasoning level without a menu.
- **`/diff`**, **`/init`**, **`/copy`**, **`/usage`** (also `/cost`), **Ctrl-R** (the last earlier entry with the words typed before it; again for older) and **Ctrl-G** (the entry in `$EDITOR`).

### Instructions and the prompt cache

- **Instruction files: AGENTS.md, CLAUDE.md and more.** `~/.llx/AGENTS.md` for every project, then in each directory from the repository's root down the first of `AGENTS.override.md`, `AGENTS.md`, `CLAUDE.md`; `@path` lines import files; a subdirectory's file arrives with the first `read` or `edit` there.
- **The context is its own cached message** before the first goal, so a new session in the same workspace reads it from the prompt cache; old screenshots are dropped together past three, so the prefix is not rewritten at every one; pictures the person attached are never dropped.

### The terminal interface

- **A browser sign-in goes on by itself.** At `/login`, once the browser comes back to the local callback, the sign-in carries on with nothing to press or paste; pasting the address the browser ended on is still there for a browser on another machine.
- **A session drawn into the scrollback.** Finished blocks go into the terminal's scrollback once; only the bottom is redrawn, in synchronized updates. Each call is a block: `• Ran cargo test`, the command wrapped under a `│`, and the start and end of its output under `└` (`… +41 lines` between), green when done and red when it failed. The answer is rendered markdown, and each task ends with `─ Worked for 48s · 6 turns · $0.0087 ───`. While a task runs: a spinner on the call, `Working (12s · esc to interrupt)`, the hands' latest actions under a browser call, and the messages queued meanwhile.
- **The prompt `❯` is a line editor:** Enter sends, Shift+Enter (or Ctrl-J, or a trailing `\`) starts a new line, a paste keeps its lines, Up and Down walk earlier entries, Alt or Ctrl with the arrows move by word, and Ctrl-A, Ctrl-E, Ctrl-U, Ctrl-K and Ctrl-W edit as in a shell. `/` lists the commands, Tab completes one, Shift+Tab toggles plan mode without Enter.
- **Slash commands and clipboard text:** Tab completes commands (adding a space for commands with arguments) or suggested mode names; Enter runs what is typed, so `/mode` and `/plan` work without arguments. Clipboard text appears as `[Paste]` in the composer and expands when sent; pasted file paths retain file and image marks.
- **Approvals and questions are choices:** `1. Yes`, `2. Yes, and don't ask again`, `3. No, and tell llx what to do differently` (a line for why), with the arrows and Enter or a number (no letter keys, so words typed as the box opens pick nothing); `ask` questions list their options and a last one to type your own answer. `Implement this plan?` is a choice too.
- **Files and pictures go in.** A path dropped or pasted into the prompt becomes `[image: …]` (a picture, sent to the model as one) or `[file: …]` (sent as its path); `@` finds workspace files; Ctrl+V attaches the clipboard's picture on macOS; `-i/--image FILE` attaches pictures to the first task.
- **Esc stops the task;** Ctrl-C stops it, clears the line, or quits when pressed twice at an empty prompt.
- **A window for each terminal session.** In a session, a terminal session `bash` keeps shows its screen in a window at the right edge of the screen (`llx · session N · <command>`), in the colours of your terminal (asked once at start, else the system's light or dark), as text only: it takes no input and never the keyboard, and what the model reads does not change. A short command opens none; `show` opens or closes it per call; closing it leaves the session running; a program that ends by itself leaves it 3 s, titled `… · exited N`. Hidden in `exec` and autonomous mode; `--show-browser` shows these windows too.
- **Over SSH the windows stay hidden** (`SSH_CONNECTION` or `SSH_TTY` set): the browser works without a window, as in `exec`, instead of opening on the remote machine's screen; `--show-browser` still shows them.
- **`exec` prints the same blocks** on stderr (in colour at a terminal, without the live part), then the answer: rendered when stdout is a terminal, as the model wrote it in a pipe.
- The release binary is about 2.0 MB.

### Fixed

- **`--timeout` holds while a task waits** on its monitors or agents: a quiet monitor no longer keeps `exec` running past its limit (it ends `exhausted`, exit 2).
- **An approval ignores keys for its first 300 ms**, so words, `j`/`k` or Enter typed for the prompt as it opens do not answer it; Ctrl-C still stops at once.
- **The key variables of a provider chosen later with `/login`** are kept from commands too, from the next command on: the list is read at each command.
- **What an agent says beside its calls** shows in its own view, as the main session's does.
- **Signed out, `exec` says how to sign in to the provider in use** (`not signed in to Anthropic: … or set ANTHROPIC_API_KEY`), not OpenRouter's variable whatever the provider.
- **A ChatGPT sign-in offers the models the backend serves alone** (the Codex and Pro models) in `/login` and `/model`.
- **The browser's decision model keeps a short timeout** (90 s) and is asked once more when it times out; chat models still are not, a reply being written is billed. Its default, `~typesafe/jev-latest`, is the app's policy and is in the catalog, with the other decision models.
- The OpenAI provider is named `OpenAI` everywhere; how it is signed in to is said where the provider is picked. `/usage` counts tokens with thousands separators, as the trace does. `/init` reads as `/init` in the transcript and in `/rewind`. `exec` with no prompt points to `llx exec "<task>"`, and a session without a terminal to `llx exec resume` too.

### Breaking

- **Renamed to `llx`, at https://llx.sh.** The command is `llx` (it was `decision-loop`); the repository is `rsaryev/llx`. The key is kept in `~/.config/llx/credentials`, sessions in `~/.llx/sessions`, and the variables are `LLX_MODEL`, `LLX_DECISION_MODEL`, `LLX_SESSIONS`, `LLX_VERSION` and `LLX_INSTALL_DIR`.
- **`--json` is schema 4: JSON lines.** One event per line replaces the single object of schema 3. The events are `run.started`, `tool.call`, `tool.result`, `step`, `turn` and `run.completed`. `run.completed` has `status` (`achieved`, `not_achieved`, `unstated`, `exhausted`, `error`, `interrupted`), `exit_code`, `answer`, `data` or `data_error`, `error`, `session`, `usage` {`agent`, `decision`, `cost_usd`} and `duration_ms`. `final_url`, `screenshot` and `expects` are gone.
- **Removed:**
  - commands: `check`, `models`, `skills`;
  - options: `--expect`, `--no-llm`, `--dry-run`, `--screenshot`, `--keep-open`, `--headless` / `--show`, `-c` / `--continue`, `--resume`, `--trace`, `--url`, `--viewport`.

  Passing a removed option is a usage error (exit 3). Their replacements:
  - `--keep-open` becomes a session: `llx`, at a terminal.
  - `-c` and `--resume ID` become `llx resume --last` and `llx resume ID`.
  - `--trace` becomes `--json` or `-v`.
  - `--headless` becomes the default: the window is invisible unless `--show-browser`.
- **The skill files are removed:** the bundled skill guides, `skills/lxsurf/SKILL.md` and the plugin marketplace.
- **New defaults:** `--max-turns` is 50 (was 30) and `--timeout` is 1800 s (was 600). `--max-turns` and `--max-steps` take 1 to 10000, and `--timeout` at most 86400 s.
- **Exit codes keep their meanings:** 0, 1, 2, 3 and 130. Exit 3 now also covers a usage error and a model error.
- **`llx login` is gone: sign in with `/login` in a session.** `login --with-token` and `login status` go with it (`llx login` is now a session whose first message is "login"). `llx logout` stays, and forgets every saved sign-in. `llx doctor` shows the sign-in to each provider. A session starts when no provider is signed in, so that `/login` can sign in; `exec` still stops with exit 3 ("not signed in").
- **`-m, --model` has no fixed default:** it is the model `/login` saved, else the provider's default (`anthropic/claude-haiku-5.5` on OpenRouter, as before).

### Changed

- **A shown browser window is an ordinary window.** It no longer floats above every other app's windows: it comes to the front when it opens, without taking the keyboard, and goes behind other windows like any other. Only the invisible (headless) window still floats, and it covers nothing.

### Added

- **The agent sees the page.** The `browser` tool takes `screenshot: true` (and `selector`, a CSS selector, to crop to one element): the WebView's own picture, canvas and WebGL included (a game board drawn on a canvas has no text to read), as a JPEG of at most 1280 pixels on its long side. It reaches the model as a user message with an image part right after the call's result, since a tool result carries text only; only the newest three pictures stay in the conversation, older ones are replaced by a line saying so. macOS only for now (WKWebView's snapshot); elsewhere the call says it is not supported. `contracts::tool::ToolOutput` has an `image`, and `contracts::chat::Message` an `Image`.
- **`ask` takes several answers** where the question allows it (`multiple`, also read as `multiSelect`): the person types several numbers (`1,3`), with a note after them if they like. `contracts::person::Answer` has `choices` (a list) instead of `choice`, and `Question` has `multiple`.
- **Tabs.** The browser keeps up to 8 tabs in its one window, a WebView each. `tab: "new"` opens `url` in a new tab, `tab: 2` switches to tab 2 first, and `close_tab: 2` closes it at the end of the call (closing the last one closes the window). A link with `target=_blank` or a `window.open` opens a tab, which is shown, instead of replacing the page. Everything else in a call acts on the tab shown; the tabs are listed in the result only when they changed, and the hands see which tab they are in but do not switch. Tabs out of sight keep running and drawing: they stand beside the window rather than being hidden. On macOS the tabs share one WebKit configuration, so one cookie store, as in a browser; a closed tab's WebView is kept blank and reused.
- **Dialogs inside web components reach the hands.** The page's controls are read through open shadow roots too, so a consent dialog built as a web component (as on play2048.co) is offered as a dialog instead of leaving its page "covered". A refused click says what covers it (`covered: a dialog "…" is on top; answer or close it first`), and the hands do not offer a covered target again until the view changes, while anything else is offered.
- **The hands stop going round in circles.** A run that keeps changing the view only to come back to the same states (a section opened and closed again) now counts as stuck, so the task ends after two such runs instead of spending its whole action budget.
- **Scripts get `press(key)` and `sleep(ms)`.** `press` sends a key the way `keys` does, with the legacy `keyCode` and `which` that games read (a plain `KeyboardEvent` from a script did not move 2048game.com's tiles); both stop a loop that runs past the script's time limit, so a bot whose answer was given up on does not play on unseen. A script whose last statement follows a block (`while (…) { … } score`) returns it.
- **Games, in the browser tool's description:** a turn-based game is played by reading the board each turn (outline, a script, or a screenshot) and pressing a few moves at a time; a real-time game by a page script that runs its own loop for at most 15 seconds and is run again. A script's 20-second limit and shadow roots are said too.
- **The browser offers controls a page made from plain elements:** a `<p>` or `<div>` with a pointer cursor, an `onclick` or a `tabindex` (as the level choices on playsnake.org), named by its text, or by its id and class for an icon. Only the outermost of a pointer group is offered, and never one that holds a real control.
- **The agent can close the browser window** with `close: true`, at the end of a call; the next call opens a fresh one. Unlike the person closing it, this does not stop the task.
- **`/login`: the provider, the model and its reasoning level.** In a session, `/login` asks for a provider (with how each is signed in to), signs in when needed, then asks for one of its models and a reasoning level. Only the levels the chosen model takes are offered, and a model switch starts at that model's default. The choice is put in at once, in the same conversation, and saved as the default for later sessions and for `exec` (`~/.config/llx/settings.json`). `/status` shows it. The choosing is a numbered list for now, behind one small picker interface.
- **OpenAI with a ChatGPT sign-in.** The `openai` provider signs in with OAuth (PKCE) in the browser, back to a local callback on `127.0.0.1:1455` (else 1457; an older sign-in waiting there is asked to give way), or by pasting the address the browser ended on. The tokens are saved in `~/.config/llx/openai.json` (0600), the access token is refreshed before it lapses and once when a request is refused, and requests go to the Responses API of the ChatGPT backend, billed to the plan. Its models: `gpt-6-luna` (the default), `gpt-6-sol`, `gpt-6.1-sol`, `gpt-6-astra`, `gpt-5.6-luna`, `gpt-5.6-sol`, `gpt-5.6-terra` and `gpt-5.5`, each with its own levels (`low` up to `xhigh`, `max` or `ultra`).
- **`--provider` and `--reasoning`** (env `LLX_PROVIDER`, `LLX_REASONING`) choose for one run what `/login` saves. A level the model does not take is a usage error that lists the ones it does. OpenRouter's catalog lists a dozen models that call tools well with the levels OpenRouter publishes for each; they start with no reasoning where they can run without, as before. Any other OpenRouter id still works, with no level.
- **Provider packages.** The providers live in `packages/providers/`: `openrouter` (moved), `openai`, and `kit`, what they share (HTTP on the OS's TLS, PKCE, the sign-in callback server and pages, private credential files, server-sent events). A provider implements the chat port (and the decision port where it has decision models) and lists its models in the `contracts` catalog; only `apps/cli/src/providers.rs` knows them. The browser's decision model stays on OpenRouter: browser actions need an OpenRouter key whichever provider the agent thinks with, and say so when there is none.

- **The browser shows itself in a session.** The window is visible by default in a session (hidden in `exec` and in autonomous mode), and the `browser` tool takes `show: true|false` per call; it never takes the keyboard. On macOS it stays out of the Dock either way.
- **Coding tools:**
  - `read` returns numbered lines, up to 2000 lines and about 20 KB at a time (below what the agent keeps of one result, so its "read again with offset" hint is always true), or a directory's entries. It refuses binary files and anything but a regular file (a FIFO, a device such as `/dev/zero`), and cuts long lines.
  - `edit` makes an exact, unique replacement (or every match with `replace_all`), or creates a file. Writes are atomic and keep CRLF line endings and permissions. A failure says how to fix the call: the text was not found, it occurs more than once, or line numbers were pasted in. It refuses a read-only file and a file that changed while its approval question was open, writes a file with other hard links in place, and creates its temporary file new, so a symlink planted there is never written through.
  - `bash` runs `sh -c` in the workspace in a session of its own, with no terminal: a program that prompts (git credentials, ssh, sudo) fails at once instead of stalling until the timeout, and git and pagers are told not to prompt or page. A timeout kills it with everything it started, without an external `kill`, and the result says "killed" only once that is so. `OPENROUTER_API_KEY` is left out of a command's environment (a command can still read it from llx's own environment or the saved key file, so this is no security boundary). Each stream keeps its first and last 5,000 bytes. Commands that leave background processes running do not hang it.
- **The workspace (`-C DIR`):** paths resolve in it, and `edit` changes files only inside it, after following symlinks. llx's saved key and its sessions are off limits to `read` and `edit`, symlinks included. On Windows the workspace is a plain path, not a verbatim `\\?\` one, so `cmd` runs in it.
- **Approvals:**
  - `edit`, `bash` and `browser` ask a gate (`contracts::gate`) before they act. The browser asks before it opens an address, hands over a goal, presses keys or runs a script, and opens only http and https addresses.
  - The question shows the whole command, never cut, with the directory it runs in when `workdir` is given (and says when that is outside the workspace). Control characters and reordering marks are escaped, so an escape sequence cannot redraw it.
  - At a terminal the person answers `y`, `n`, `n <why>` (the reason reaches the model) or `a` (always, for that tool). Only the bare word approves, and what was typed before the question appeared is discarded. The question goes to the terminal itself, so this works with piped stdin, `--json` and stderr sent to a file.
  - `-y` / `--yes` allows everything (it is `--mode bypass`).
  - With no terminal and no `-y`, effects are refused, and the model is told to have the person rerun with `--yes` or `--mode bypass`. In plan mode it is told instead to plan from reading files: a read-only request never points to a mode that changes files unasked.
- **Modes and questions:**
  - `--mode default|plan|bypass|autonomous` (also `--permission-mode`), one setting for what runs without asking. `default` asks at the terminal; `plan` refuses file changes in code and asks for every command (a terminal session's start and each input too) and browser action, with its own "always" answers; `bypass` allows everything (`-y` is `--mode bypass`, and giving both is a usage error); `autonomous` allows everything and answers no question. A mode changes only what the gate and the person answer and the context's text; the tool list stays the same.
  - In a session, `/mode [name]` (also `/permissions`) lists or switches the mode, `/plan [task]` enters plan mode (and runs the task), and a line that is only Shift+Tab, then Enter, toggles plan mode. The prompt shows the mode when it is not `default`, `/status` shows it and any terminal session still running, and the model is told of a change before its next task, in a `<mode_change>` note.
  - When a task in plan mode ends well, llx asks "Implement this plan?": `y` goes back to the mode plan mode was entered from and runs "Implement the plan."; `n` or Enter stays; anything else is the next entry, so typing a change revises the plan.
  - The `ask` tool, in a session only (not in `exec`): one to three questions with numbered options, answered by number, number and note, or in the person's own words; Enter skips. In `autonomous` mode it fails at once with a reason, and the agent goes on with its best assumption.
  - Ctrl-C ends an open question or approval at once, instead of after the next Enter.
  - Time spent waiting on the person (an approval, a question) does not count toward `--timeout`.
  - The `<environment>` block's `Approvals:` line is now `Mode:`.
  - No `--json` schema bump and no exit-code change: `--json` needs `exec`, where `ask` is not offered.
- **Terminal sessions, inside `bash` (Unix):**
  - `tty: true` runs a command in a terminal of its own (a PTY from `openpty`, the shell its session leader with the PTY as its controlling terminal, and SIGINT and SIGQUIT back to their defaults, so Ctrl-C works even when llx was started in the background with them ignored), for a program that must ask: a REPL, an installer's questions, a login. The call returns once the output settles, `wait_for` text appears, the program exits or `yield_ms` passes (2 s by default, 15 s with `wait_for`, 0.1 to 60 s), with a session id while it runs.
  - `session` continues one: `input` types text (a line break is Enter, at most 4096 bytes and 1000 on one line, since a terminal drops the rest of a longer line, Enter included), `keys` presses named keys (Enter, Tab, Escape, Backspace, Space, arrows, Home, End, PageUp, PageDown, Delete, F1-F12, Ctrl-<letter>; arrows follow the program's cursor-key mode), `session` alone reads the output since the last call, and `close` kills it with everything it started.
  - The terminal is 120×40 with `TERM=dumb`, `NO_COLOR=1` and no pager. Output is a transcript, not a screen: control sequences are dropped, a carriage return's redraw keeps its last state, and cursor and status queries are answered so a program that asks does not hang. Full-screen programs are not supported.
  - At most 4 sessions run at once. Every session ends with its task (its answer, `--timeout`, Ctrl-C), on `/new`, and when llx exits; a second Ctrl-C kills them before quitting. Ending a session (or a command at its timeout) kills every process of its process session, found by session id, not only the shell's group: the jobs an interactive shell puts in groups of their own (`npm run dev &` in `bash -i`) die with it. Only a process that leaves the session itself (`setsid`) outlives it.
  - Starting a session asks the gate (`python3 -q  (in a terminal)`), and so does each `input` or `keys`, showing exactly what will be typed; reading and closing do not ask. Invalid combinations are refused before anything is asked or typed.
  - A call without `tty` or `session` runs as before. On Windows the fields are not offered, and `tty` is refused with a reason. No `--json` schema bump: tool arguments were already free-form.
- **Context:** the agent starts with an environment block: the workspace, the OS, the date, the mode, and whether a browser is available (the shell is named in `bash`'s own description). After it come the project's `AGENTS.md` files, from the git root down to the workspace (16 KiB each, 32 KiB in all). A resumed session also gets its earlier tasks, each capped (a piped `<stdin>` block left out), and the directory they ran in.
- **A command line: a session, or one task:**
  - `llx` opens a session at the terminal, and `llx "<task>"` opens one that starts with that task. Each line typed goes on in the same conversation. Slash commands: `/mode` (also `/permissions`), `/plan`, `/new` (also `/clear`), `/status`, `/help`, `/exit` (also `/quit`, Ctrl-D).
  - `llx exec "<task>"` (alias `llx e`, or `llx -p "<task>"`) runs one task and exits. `llx exec - < task.md` reads the prompt from stdin, and piped stdin is added to a prompt as a `<stdin>` block; llx says on stderr when it reads stdin that may stay open (`</dev/null` skips it).
  - `llx -c` goes on with the newest session of this directory, like `llx resume --last`; `llx exec resume [ID | --last] "<task>"` runs one more task in a saved session.
  - `--json`, `--schema` and `-o` answer one task, so they need `exec` or `-p`. A session without a terminal on stdin is a usage error that points to `llx exec`.
  - `-y` is also accepted as `--yolo` and `--dangerously-skip-permissions`.
  - `-o FILE` also writes the last answer to a file (the data with `--schema`; empty when none matched), before `run.completed`, so its `exit_code` is the process's even when the file cannot be written. `-v` shows results and per-turn usage.
  - With `--schema`, a model that answers in text twice ends the run as `not_achieved` (exit 1) with `data_error`, and nothing on stdout.
  - The trace goes to stderr and the answer alone to stdout.
- **Commands:** `login --with-token` (at a terminal, the key is not echoed), `login status`, `resume [ID | --last] [PROMPT]`, and `doctor`, which now also checks the shell. Sessions record their workspace: `sessions` shows it, and `resume --last` takes the newest session of this workspace. A saved key file is narrowed to mode 0600 even when it existed before.
- **A session** reads a line that is not UTF-8 instead of quitting, and says why when its input fails.
- **Stopping:**
  - `--timeout` stops a task between model turns (exit 2), never in the middle of a tool call.
  - Ctrl-C (or SIGTERM) stops the task: the running command is killed with everything it started, and so is every terminal session, nothing more is approved, the browser's hands stop, and the task ends as `interrupted` (exit 130) with its `run.completed` event and session. A second Ctrl-C quits at once. Closing the browser window stops the task the same way instead of killing the process; the session goes on, and the window opens again when needed.
  - With `--json`, a closed stdout ends the run quietly at its next turn (exit 0).
  - Without a display (headless Linux), llx runs without the browser tool instead of failing.
- **The agent:**
  - A new system prompt for coding work: plan briefly, read before editing, make the smallest correct change, validate with the project's own checks, and report what changed and what was verified.
  - Observer hooks for calls, results and turns.
  - A `Fatal` call, a returned `result` and a final schema mismatch are now recorded in the conversation, so a follow-up never sees a call marked as not carried out.
- **Evals:** six coding tasks on small fixtures (write and run a script, fix a failing test, rename across files, answer a question about the code, answer a script's questions in a `tty` session, plan a feature in `--mode plan` without changing a file), each judged by a `check` command run afterwards. A task can pass more llx arguments (`args`, such as `["--mode", "plan"]`, which then replace `--yes`) and require a tool call (`uses`, by its arguments, the ones it must have, or patterns they match) or rule one out (`avoids`). The summary prints totals per kind: pass rate, median and p90 duration, mean turns and tool calls, total and mean cost, and input, cached and output tokens. Eight browser tasks are kept, judged by the exit code and the answer's contents (`contains`, `matches`). Every run works in a fresh temporary directory.
- **Project:**
  - CI runs on macOS, Linux and Windows, with an MSRV check of every target (1.88) and a size check (2.5 MiB). Third-party actions are pinned to commits.
  - A tag `vX.Y.Z` runs the tests on every platform, then builds release archives with SHA256SUMS and build-provenance attestations, plus `scripts/install.sh` (which runs nothing until it is downloaded whole) and `scripts/install.ps1`.
  - New `CONTRIBUTING.md`, `SECURITY.md` and `RELEASING.md`.

### Fixed

- **Secrets:** `bash`, terminal sessions, agents and `jobs monitor` no longer see the key variables of the provider in use (as the catalog names them, such as `DEEPSEEK_API_KEY`), nor `OPENROUTER_API_KEY`; only OpenRouter's was left out before. Other providers' variables stay, so a project's own tests can use them. A provider chosen later with `/login` is not added, and hiding is still no barrier.
- **The conversation:**
  - A screenshot taken in a turn with several calls comes after all of that turn's results; chat-completions hosts rejected the request with a 400 before.
  - The context estimate counts the tool specs, so fresh reads are no longer elided early in a session.
  - Failed calls in one parallel turn count as one failed turn; before, three of them took the tools away for the rest of the run.
  - A result without state no longer drops the newest page state.
  - An answer on the last turn is kept, and the run no longer waits on agents it cannot read.
  - What the model writes beside its tool calls, or while it waits on agents, is shown in the session and in the stderr trace. `--json` is unchanged (schema 4).
- **Agents and jobs:**
  - A monitor's output can no longer close its own `<monitor>` tag, nor an agent's answer its `<agent_result>` tag. A monitor batch that starts a task of its own says that it is data, not the person's instructions, and the `jobs` description says `<monitor>` messages are untrusted.
  - `/new` ends the old conversation's monitors and schedules, and nothing more from them reaches the new one; a new monitor may take an old one's name.
  - `/rewind` of the conversation lets the session's agents go (their names and the 6-agent cap are free again), lists background tasks as the transcript shows them (`≋ ticks · tick 1`, `◷ hello · say hello`) and does not put them back in the prompt.
  - `x` on a schedule's row removes the row, and the chosen row in the list is always one that is drawn and still there.
  - The "Waiting for …" line is said once per wait.
  - An agent stopped before it answered tells the main agent so, and that `send` goes on with it; a message an agent ended before reading is reported as unread.
  - A refused call is drawn as refused even when another call failed meanwhile.
- **Terminal sessions:** stopping one agent's wait kills only that session ("stopped after Ns: session N was killed, with everything it started"). The limit of 4 holds with parallel calls: a session in use by another call counts, and a call on it is told it is busy.
- **Tools:**
  - A `bash` workdir that is a symlink out of the workspace is shown with where it really leads, and as outside the workspace.
  - `edit` says why `old_string` cannot match across lines in a file that mixes `\r\n` and `\n` endings.
  - `browser`: a wrong `tab` is refused before the person is asked; `show` no longer reloads a window already shown or hidden that way; an address with no scheme on this machine (`localhost`, `127.*`, `0.0.0.0`, `[::1]`) opens over http.
- **Providers:**
  - With a ChatGPT sign-in, reasoning "off" is sent as effort `none`; the backend reasoned at its default before. On OpenRouter, a model that takes no levels is sent no `reasoning` field.
  - A chat request that times out is not sent again, so a long reply is not billed three times (a connect or DNS timeout is still retried); the chat-completions timeout is 600 s, as on the ChatGPT backend.
  - A sign-in callback is not held up by a browser connection that says nothing.
  - Credential files and the cached catalog are written whole (a temporary file, then a rename). A cached catalog cut short is fetched whole again, and a 304 no longer rewrites the cache.
  - A refused key reads "<Provider> refused the key: …; type /login, or check the key's variable".
- **The interface:**
  - The "ctrl-c again to quit" hint goes once its 2 s window has passed, and quitting leaves no prompt row on the screen.
  - Shift+Tab and Shift+Up/Down pressed during a task are queued as `shift+tab · next mode` and `more/less reasoning`, not as escape codes; queued monitor and schedule results read as in the transcript.
  - Esc closes the command list `/` opens. `/init` shows as `/init`. `/copy` copies the last answer that says something. A wide row of a table note (`/mode`, `/help`) wraps under its second column.
  - The closing rule says "Ran out after" when a limit ran out and "Not achieved after" for a goal not achieved.
  - Shift+Up/Down says "Reasoning: high (saved)."; a model with no levels says it takes none.
- **The command line:**
  - `--image` naming a file that is missing or not a picture is a usage error (exit 3) before any model is called; it was skipped before.
  - `resume` and `exec resume` take the prompt unquoted, like `exec`; with `--last`, every word is the prompt.
  - `doctor` fails the decision-model check when the model gives no answer.
  - `--help` names the right sessions folder, `~/.llx/sessions`, and the prompt line's read error starts with `llx:`.
- **Tests:** unit tests use the built-in model catalog, not the one fetched to the machine; integration tests run the binary with a cleared environment; a new `suite/exec.rs`; `cargo nextest` works (`.config/nextest.toml`); `scripts/coverage.sh` (cargo-llvm-cov with nextest, failing under 80% of lines) runs in CI; build scripts count toward the line budget.

### Browser

The browser tool and its hands, rebuilt since 0.1.0 as an agent's tool:

- **The agent drives the hands.** The agent hands short tasks with the exact `values` to type to the hands: the decision loop on `--decision-model`, default `~typesafe/jev-latest`. It checks every page itself.
- **Tool results come in sections** (`### Helper`, `### Page (untrusted content)`). The page arrives as an outline: headings and text in reading order, and every control with its ref, value and state. The toolbar (Back, Reload, page size) is part of the view.
- **More actions:** the tool presses keys (`keys`, `key_delay_ms`), runs a page script, and reads the whole text. WAIT takes a length the decision model picks (1 to 30 s), and ends early when the page changes.
- **Steadier on real sites:**
  - A filled field and a placed value are not offered again.
  - Consent banners inside same-origin frames are seen and accepted. A click under a banner that loads late is decided again.
  - A script or a click that leaves the page no longer waits for a reply that cannot come. A parsed document is used after at most 1 s more, instead of waiting up to 20 s for every ad and video.
  - The page's own requests in flight mark it busy. A DONE the model is unsure of, or one sent while the page is still fetching, is decided again only if the page changes.
  - A link inside a heading keeps its ref.
  - A `<button>` is named by its text, not its `value`.
  - The hands do each action a goal names once, and the tool asks for an end state the page will show ("question 2 shows", not "the next question"). A quiz is no longer answered past the question asked.

## 0.1.0

First release, as a browser CLI with a chat model that planned and a decision model that clicked and typed: `check` and `--expect`, `--json` and `--trace`, stable exit codes, headless by default under CI, OpenRouter sign-in, viewports, cost reports, key presses, page scripts, date pickers, guides for agents.
