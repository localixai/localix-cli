# AGENTS.md

Instructions for AI coding agents working on this repository.

**ARCHITECTURE.md is binding: follow it strictly.** Read it before any change. It fixes the
layers (today's and the daemon's target), what may know what, and the rules by id (L, C, P, A,
K, W, R, S). Every new file goes where its layer says, in its target crate's place. A change that
does not fit is redesigned, or ARCHITECTURE.md is changed first, in its own commit, with the
owner's agreement; an exception is never made in code. Cite the rule ids a change touches.

`llx` is an ultrafast, lightweight coding agent for the terminal. It has seven tools: `read`, `edit`, `bash` (commands run to their end), `window` (everything it opens, as tabs of one window: native browser pages and terminal sessions, each kind a strategy, a goal on any tab carried out by a decision model), `ask`, a question to the person that the app answers, `agents`, other agents it starts (each a session with a parent, run in parallel), and `jobs`, monitors and schedules. The tool set is deliberately minimal: no plugins, no MCP, no skills, no extra tools. A new capability goes into one of these seven (as an `action`, or a kind of `window` tab), or is left to the shell.

## Layout

Packages and an app. Every dependency points inward (rules L1-L5 below).

- `packages/contracts`: ports only, no logic.
  - `chat.rs`: the chat-model port, with our own `Message`, `ToolCall`, `Request`, `Completion` and `Usage`.
  - `catalog.rs`: what a provider offers: `ModelEntry` (an id, a name, the `Effort` levels it takes, its default) and `Selection`, a model with a level it takes (its fields are private: no other level can be put in).
  - `tool.rs`: the agent's tools: `ToolSpec`, `ToolOutput { report, state }`, `ToolError { Failed, Fatal }`.
  - `gate.rs`: a tool asks before an effect: `Effect { tool, summary }`, `Verdict { Allow, Deny(why) }`, `Gate`, and `Open`.
  - `person.rs`: a tool asks the person: `Question` with its `Choice`s, `Answer`, `Reply { Answers, Nobody(why), Stopped }`, `Person`.
  - `view.rs`: what the hands drive: `Observation` with an outline and refs, `Action`, `View`.
  - `model.rs`: the decision-model port: typed questions, `Decider`.
  - `hands.rs`: a `Task` carried out on a view, and its `Report`.
  - `history.rs`: sessions, `History`.
  - `inbox.rs`: what reaches a run between its turns, `Inbox`.
- `packages/agent`: the agent.
  - `agent.rs`: the loop: a turn's calls (those that may, at once: `calls.rs`), the inbox between turns, the stop conditions. `Agent::context` is the app's environment block, sent before the first goal; `Agent::tell` is what the app says between goals (a changed mode), naming no tool. `agent/handling.rs`: a turn's calls carried out, answered and counted; a `result` checked against its schema.
  - `spend.rs`: `Spend`, what the chat model spent in a run.
  - `observer.rs`: what a run shows as it goes: calls, results, turns.
  - `conversation.rs`: messages that only grow. Older states are dropped past a budget, and open calls are closed; `truncate` for a rewind.
  - `history.rs`: how a conversation is prepared before each request, a `Strategy` the app chooses: `Pipeline` (stages `superseded`, `outputs`, `arguments`, behind a 90%→60% gate of the model's limit, a protected tail, elisions that stay: made for the prompt cache) or `Keep`. `history_tests.rs` simulates a 300-turn dialogue.
  - `schema.rs`: `--schema` as the `result` tool.
  - `answer.rs`: the `RESULT:` line.
  - `prompt.md`: the system prompt.
  - `fakes.rs`: test-only fakes: a scripted chat, an echo tool, a recorder.
- `packages/decision-loop`: the hands.
  - `decision_loop.rs`: resumes a run until DONE, BLOCKED or the budget runs out; implements `Hands`.
  - `goal.rs`: one run (observe, decide, act) and its guards.
  - `ask.rs`: the question each step asks, with `offer.rs` (the controls in scope, the operations they allow, the pairs worth typing, the covered targets).
  - `step.rs`, `choice.rs`.
  - `meter.rs`: the decision model's spend.
- `packages/tools`: the tools, a module each.
  - `workspace.rs`: where paths resolve. A file can be read anywhere but the paths the app denies (its config directory with the keys and tokens, its sessions), and changed only inside the root. Its commands run with the client's environment when it has one (`with_env`).
  - `fs/`: `read.rs` (numbered lines, or a directory's entries) and `edit.rs` (exact replacement, or a new file), with `text.rs` (numbering, line endings, +/- counts) and `atomic.rs` (temporary file, then rename).
  - `shell/`: `bash.rs` (the tool), `process.rs` (`Env`: the variables a command runs with, the place's else the process's, less the hidden ones; its own session with no terminal, a deadline, a stop, both pipes read on threads, the group killed, then the rest of the session), `sweep.rs` (every process of a session killed by its session id, with system calls on the stack only, so the signal handler may use it) and `output.rs` (head and tail kept, the middle cut).
    - Terminal sessions (`tty`, `session`; unix only): `interactive.rs` (the arguments checked, the gate asked, the call carried out), `report.rs` (what the person and the model read), `pty.rs` (`openpty`, the shell its session leader with the PTY as its terminal, the reader thread, guarded writes), `stream.rs` (the output since the last call, the `wait_for` match), `filter.rs` (control sequences dropped, `\r` redraws, DSR replies, DECCKM), `keys.rs` (named keys), `session.rs` (`Sessions`, the registry the app owns: at most 4) and `running.rs` (one session's event-driven wait and its end, and `kill_all_now` for the signal handler: 32 slots, for every registry of the process; `running_anywhere` counts them). With a window: `screen.rs` and `screen.html` (the screen a window shows: only text and line moves reach the emulator, rows set as text) and `watch.rs` (`Watch`, a window the app gives; the title, the 3 s linger after an exit).
  - `browser/`:
    - `tool.rs`: `BrowserTool`, the agent's `browser`.
    - `page.rs`: `Page`, the window as a `View`.
    - `host.rs`: the tao event loop on the main thread; a closed window fails the calls waiting on it and opens again when needed. With `host_events.rs` (the loop and the window's commands), `host_web.rs` (the browser's commands on its tabs) and `host_handle.rs` (`Browser`, the work thread's handle).
    - `webview.rs`: the driver, with `webview_driver.rs` (each `Driver` call on the page, and the load it starts followed).
    - `toolbar.rs` (Back, Reload, page size) and `viewport.rs`.
    - `platform.rs`: what differs by OS.
    - `guard.rs`: what may load, and the probe's token.
    - `script.rs`.
    - `tabs.rs` (the window's tabs, one WebView each; a closed tab's view kept for reuse), `tab_view.rs` (how a tab's WebView is built), `args.rs` (the tool's arguments, and what of a call the gate reads) and `capture.rs` (the compositor's picture of the page, as JPEG).
    - `deck.rs`: the one window: the browser's tabs and the terminal sessions' screens as its tabs, a strip along its top (`strip.rs`, with `strip.html`: the tabs named, a click switches or closes one, read as data; with several projects' tabs, each project's together, its name before them); what the agent does in the browser shows the browser again; it closes when nothing is left in it. `panel.rs`: a tab that shows a caller's page and runs the frames painted into it (a terminal session's screen): no input, no navigation, never the keyboard.
    - `probe/*.js`: the page side: snapshot, frames, outline, actions and settle, keys, evaluate.
  - `window.rs`: `WindowTool`, the agent's `window`: `open` (one tab or several), `interact`, `screenshot`, `close`, and where the window shows `list`, `show`, `resize`; a `goal` on any tab, handed to the `Hands` with the tab's view. It knows no kind: each is a `Kind` (open, interact, snapshot, screenshot, close, tabs, view), and the window the person sees is `Windowed` (told of the tabs a call opened, it puts them in its holder's group; `list` names a tab's group only beside another's). The app gives the kinds (`apps/cli/src/kinds.rs`) and the window (`apps/cli/src/windows.rs`, `Tabs`). With `window_goal.rs` (a `goal` handed to the helper, and its report) and `window_tabs.rs` (tabs opened at once, closed, listed, found by id).
  - `ask.rs`: `AskTool`, the agent's `ask`: one to three questions, read leniently, answered through `Person`.
  - `testing.rs`: test-only: temporary directories, and a gate that records what it is asked.
- `packages/providers/`: the models' providers, a package each, told who calls them (`App`).
  - `kit/`: what they share, with no port: `http.rs` (`Http`, a POST answered with a status and a body; `Web` on the OS's TLS; forms and queries), `oauth.rs` (PKCE, random state, a pasted redirect, a token's claims, opening the system browser), `loopback.rs` (the sign-in callback on 127.0.0.1: route and `state` checked, `/cancel` to give way), `page.rs` (the browser's success and failure pages), `store.rs` (credential files, 0600), `sse.rs` (server-sent events).
  - `models/`: the model catalog, the one source of model facts (MODELS.md): `data/` is models.dev's own files for the popular providers (`scripts/models_sync.py` copies them from a clone), `build.rs` resolves them as models.dev does into its v2 shape, `lib.rs` types it (providers, chat and decision models, windows, levels and defaults, cache flags, API addresses), `source.rs` fetches models.dev's v2 API at most once a day (ETag) and keeps it in `<config>/models.json`, over the built-in files.
  - `openrouter/`: `chat.rs` (chat completions, tool calling; OpenRouter's `reasoning`, cost and prompt-cache marks, or `reasoning_effort` for any other OpenAI-compatible host), `system_one.rs` (the decision models), `client.rs` (OpenRouter, or any address the catalog gives; a timed-out request is not sent again, but a decision is, once), `auth.rs` (the OpenRouter key: environment, pasted, or a PKCE sign-in).
  - `openai/`: a ChatGPT sign-in: `auth.rs` (the OAuth flow: authorize, exchange, refresh), `tokens.rs` (the tokens, the account id from their claims, `openai.json`), `responses.rs` (the chat port over the ChatGPT backend's Responses API, streamed; refresh before expiry and once on 401).
  - A new provider is a package here implementing `Chat` (and `Decider` where it has decision models) with a catalog, and one arm per `match` in `apps/cli/src/providers.rs`.
- `packages/protocol`: what the interface and the server say to each other (ARCHITECTURE.md W1-W8), wire types only, no workspace package: `event.rs` (`Event`: what happened in a session; a call's `Label` and `Outcome`), `asked.rs` (`Asked`: a question for the person, `Reply`), `command.rs` (`Command`: what the interface asks of the server, `Done`; `Place`, where a client is: its directory, environment and colours, given at `Initialize`), `frame.rs` (`ToClient`, `ToServer`: every frame, `resource/verb` named, the same in process and over a socket).
- `packages/client`: the interface's side of the seam: `Client` (`in_process` over channels; `connect`, unix only, over a socket in `socket.rs`: a frame per JSON line, a writer thread, the same reader loop), `call` waits for what a command came to (`CallError`: gone, timeout, failed), `send`, `reply`; every other frame goes to the interface as it comes. Only `protocol`.
- `packages/queue`: one queue for what reaches a run from outside its own calls: entries with their own policy (whether a running task takes one, and what becomes of it while none runs), bounded, rendered as tagged data. No workspace package.
- `packages/session`: a history store: sessions as JSON files.
- `apps/cli`: the `llx` binary and the composition root.
  - `main.rs`: where the process exits (or the browser host, once its event loop owns the main thread, with the code the run returns).
  - `cli.rs`: the command line.
  - `commands/`: `run.rs` (a session or one `exec` task: checks, then the browser host, then the work) with `run/task.rs` (one task on the `Rig`: run, reported, recorded), `repl.rs` (a session's prompt, in the interface, and the plan handoff), `entry.rs` (what is typed at the prompt: the slash commands, the help), `ended.rs` (how a task ended: the report, the exit code, the decision model's spend), `login.rs` (`/login`: the provider and its sign-in, each a pick, then `model.rs`'s steps), `model.rs` (`/model`, the reasoning level, Shift+Up/Down; a choice put in, its key's variables kept from commands, saved), `signin.rs` (signing in to each provider, and `logout`), `sessions.rs` (`sessions`, `resume`), `doctor.rs`, `rewind.rs` (`/rewind`), `extras.rs` (`/diff`, `/init`, `/copy` (the text sent as `Event::Copy`), `/usage`).
  - `providers.rs`: the one place that knows the provider packages: `Provider` is a catalog id; its sign-ins and keys (its variables, else `key-<id>`), building its chat model (the ChatGPT backend, else chat completions at the catalog's address), the models it offers (with a ChatGPT sign-in, those the backend serves), the catalog's refresh; the decision model (OpenRouter's) and its default, `DECISION_DEFAULT`, a catalog id.
  - `choice.rs`: the provider, model and level a run uses (the options, else what `/login` saved in `settings.json`). `live.rs`: the models a session thinks with, switched by `/login` between tasks.
  - `picker.rs`: `Picker` (a title and items in, an index or a cancel out) and `Lines`; the interface implements both.
  - `agents.rs`: `agents`, the tool that starts other agents, and the `Registry` it shares with their threads (scoped to the session in `compose.rs`): each a session whose parent is the main one, with the same context and gate, a stop of its own, and neither `agents` nor `ask`. With `agents/spec.rs` (the description and the arguments), `agents/child.rs` (one task of an agent on its thread, recorded in its session, its answer posted), `agents/kit.rs` (each agent's own tools; one browser action at a time), `agents/inbox.rs` (the main agent's inbox: steering, then agents' answers, then jobs; an agent's letters) and `agents/watch.rs` (its calls to its own view; its approvals named).
  - `jobs.rs`: `jobs`, monitors and schedules: what comes back goes to the task running, else to the interface as a task of its own. `jobs/monitor.rs` (a command in its own process group, lines in batches), `jobs/cron.rs` (5-field cron in local time, delays), `jobs/spec.rs`.
  - `meta.rs`: what is known of a session beside its tasks, as it changes (`Update`: its title now, more fields later), reaching the interface as `Event::Updated` and the saved session through `Pending` (applied before each of the lane's saves, written at once to a session on disk, under one lock). `meta/title.rs`: the title, asked of the same model at its lowest level on a thread of its own when a session's first task starts; the task never waits for it.
  - `arrivals.rs`: the one table of what reaches a lane's run (an agent's answer, a monitor's lines, a schedule, what the person did in the window), each kind's policy and tag, the lane's queue, and `Lanes` (every lane's queue, held weakly, for what the window says). `kinds.rs`: the kinds of `window` tab, strategies over the engines: `Browser` (the browser tool and its page; one browser action at a time) and `Terminal` (`kinds/terminal.rs`: `bash`'s sessions, `TerminalView` for a goal).
  - `checkpoints.rs`: each task's files as it found them (`edit`'s changes, the agents' too), for `/rewind`.
  - `compose.rs`: builds the models, then (`assemble`, testable with fake models) the hands, the mode's gate and the person, then (`equip`) the tools, the agents, the jobs and the inbox, then the `Agent`; `Rig` switches the mode and tells the agent of a change. With `compose/setup.rs` (`Setup`, what the lanes share, a lane's `Seat`), `compose/lane.rs` (a lane's models, stop and output) and `compose/rig.rs` (`Rig`, built by `drive`, which runs the work with it).
  - `context.rs`: the environment block, the instructions, an earlier session; one message, the same all day, cached.
  - `instructions.rs`: `~/.llx/AGENTS.md`, then per directory `AGENTS.override.md`, `AGENTS.md` or `CLAUDE.md` with `@imports`; `Instructed` adds a subdirectory's file to the first `read`/`edit` there.
  - `attach.rs`: pictures attached to a message, read for the model (the paths the interface sends, `--image`).
  - `server.rs` and `server/`: the server's side of the seam (ARCHITECTURE.md W1-W8; to `apps/server`): `ui.rs` (`Peer`, what the server holds of the interface: frames out, the questions waiting by id, the lanes' inputs; `Ui`, a session's port: reports in its name, questions with an id that a stop withdraws, entries from the server's queue), `inputs.rs` (each lane's queue: the entry for the lane at its prompt, words for the task running while one runs, commands and Tab for after, the background behind the person; told to the interface as `Event::Queue`), `serve.rs` (each frame from the interface: replies to their questions, commands done and said done; the place `Initialize` gives goes to the desk), `place.rs` (where a client is, as its sessions take it: the workspace at its directory with its environment, its colours), `asking.rs` (the gate, the `ask` tool's person and `/login`'s steps over `Ui`), `label.rs` (how a call reads, said by the server, which knows the tools), `wire.rs` (domain values as their wire form; how a task reads, `shown`), `socket.rs` (unix: `bind` in a 0700 directory, the socket 0600; `accept`, each connection's ends on a thread of their own; per connection a reader that drops and says a line that is not a frame, and a pump into a bounded writer's queue that cuts off a client that does not read).
  - `daemon.rs` (unix): where the daemon is found (`socket_path`: `$TMPDIR/llx-<uid>/<FNV-1a of the config directory>.sock`), `lock` (`flock` on `<config>/daemon.lock`, a second holder refused) and `probe` (a client's call answered within a wait).
  - `tui/`: the session's interface: history into the terminal's scrollback, a live area, the composer, choices. the interface knows the sessions only through `protocol` and `client` (L7, checked): `tui/mod.rs` starts it (the handshake: the commands and modes it offers; the key reader, the client), `tui/drawing.rs` is its thread (frames and keys in, `Out` commands and replies sent, drawn), `tui/terminal.rs` (the terminal's modes, the keys, `$EDITOR`), `tui/spend.rs` (the session's spend at the footer's right), `tui/clipboard.rs` (`/copy`'s text into the person's clipboard), `tui/printable.rs` (model text escaped before it reaches the terminal), `tui/attach.rs` (what the person attaches: a dropped path, Ctrl+V's picture, kept under `~/.llx/pasted`); `tui/views.rs` keeps a transcript per session (each lane, with its own entries and queue, and its agents), and `tui/agents.rs` routes the events to them and switches the screen between them (`tui/agents/ends.rs`: what a call's and a task's end add); `tui/tree.rs` draws under the composer the lane shown with its agents (time and tokens) and the jobs, the other lanes counted on one line (`tui/tree/row.rs`: one row drawn). `tui/composer.rs`: where the person types, with `composer/marks.rs` (attachments' and pastes' marks) and `composer/render.rs` (its rows and cursor); `tui/select.rs`: a choice put to the person, with `select/render.rs` (it drawn). `tui/queue.rs`: the questions, one open at a time, a background lane's labelled, its own choices held until shown. `tui/sessions.rs`: the list ← opens (lanes open, "+ New session", this directory's saved sessions; `x` stops a lane); `tui/opening.rs`: at most `MAX_LANES`, the oldest idle lane closes for a new one; `tui/quitting.rs`: the `Quit llx?` question while work runs. `tui/test_support.rs` (test-only): an `App` driven with events and keys, drawn through the real `Screen` into a fixed-size `vt100` terminal, spinners and durations normalized.
  - `windows.rs`: whether windows show (a session, not autonomous, not over SSH, or `--show-browser`), and the terminal sessions' windows as the browser host's panels; a lane's tabs are of its group, its directory's name; `colours.rs`: the person's terminal colours (OSC 10/11/12), asked once before the interface reads the keys, and sent with its place.
  - `mode.rs`: the four modes (`default`, `plan`, `bypass`, `autonomous`), the context's `Mode:` text, the `<mode_change>` note, and `ModeGate`: what each mode allows, refuses or asks.
  - `gate.rs`: asking at the terminal (`Tty`, `TerminalGate`, a wait Ctrl-C ends); nothing once the task is stopped.
  - `interrupt.rs`: Ctrl-C, SIGTERM and a closed window stop the task, not the process. `Process` (a generation each stop raises) for the signal handler; an `Interrupt` per session, so stopping one leaves the others; the stops by session id (`register` returns a `Known` that forgets it when what it stops ends; a job is known by `job_key(lane, id)`), for the interface's keys and `x`.
  - `desk.rs`: the lanes of a session at the terminal, each a whole session on a thread of its own over `compose::Shared`; opens one per request (`/new`, a saved session from the list; never two for one id, at most `MAX_LANES`), answers the list with the saved sessions, ends when the last lane does. `desk/ending.rs`: the lanes table, the guards that end them however they end, and "Quit anyway" (every lane stopped, at most 2 s, then the process ends). `desk/test_support.rs` (test-only): lanes on gated models and a person at a real `App`.
  - `brake.rs`: stops between model turns, on `--timeout` (not counting time spent waiting on the person) or when stdout's reader is gone.
  - `input.rs`: the prompt from arguments and stdin, and `--schema`.
  - `output.rs`, with `output/human.rs` (the trace on stderr), `output/json.rs` (`--json` events) and `output/observer.rs` (`Output` as the agent's `Observer`).
  - `error.rs` (`Failure`) and `exit.rs` (the exit-code contract).
  - `testing.rs`: test-only helpers.
  - `tests/all.rs`: the one integration-test binary. `common/`: a home per test, a scripted model, a pseudo-terminal read through `vt100`. `suite/`: the binary end to end, with no network (`suite/tui/`: flows at a real terminal, unix only); `suite/repo/architecture.rs`: the layer rules; `suite/repo/budget.rs`: the size budget.
- `evals/`: live tasks, both coding (fixtures in `evals/fixtures/`) and browser (real sites). `benchmarks/`: their history.

## Rules

Layers (checked by `apps/cli/tests/suite/repo/architecture.rs`):

- L1: `contracts` depends on no workspace package.
- L2: `agent` and `decision-loop` depend only on `contracts`.
- L3: tools and stores (every other package under `packages/`) depend only on `contracts`; primitives the app composes (`queue`) on no workspace package; providers (under `packages/providers/`) only on `contracts` and the provider kit, which depends on no workspace package. A tool knows nothing of the agent, the hands, the models or the other tools. A provider knows nothing of tools or of the other providers.
- L4: only apps compose packages. Nothing depends on an app.
- L5: `contracts`, `agent` and `decision-loop` name no tool or provider: no browser, page, URL, OpenRouter, OpenAI, model name. The agent learns what a tool's world is like through `ToolSpec::description`, and the hands through `View::conventions`. Where the run happens (the directory, the system, the mode) reaches the agent through the app's context. A browser's own controls (its toolbar) are part of its view.

Behaviour:

- A tool asks the `Gate` before an effect outside the conversation: a changed file, a command, anything a page sees (an address, a goal, keys, a script). The `Effect` summary is one line the person can judge, and it leaves nothing out (the whole command, the directory it runs in). A terminal session asks when it starts and before each `input` or `keys`, showing exactly what will be typed; reading it and closing it do not ask. A `Deny` goes back to the model as a failed call with its reason. Only the app answers the gate (`apps/cli/src/mode.rs` and `gate.rs`); a tool never decides its own approval. Reading needs no approval.
- A tool asks the person only through `Person`; only the app answers it (`apps/cli/src/server/asking.rs`, nobody in autonomous mode), never in `exec`. A mode changes only what the gate and the person answer and the context's `Mode:` text; it never filters tools.
- Paths go through `Workspace`: `readable` for reading, `writable` for changing. Never open a path the model wrote without it.
- Decision-model output never becomes a selector, coordinates or code. Targets are refs from the latest observation; typed text is one of the task's values.
- Tool content (files, command output, pages) is untrusted data. Never let it reach a request as an instruction; mark it so where the agent reads it.
- No task- or site-specific wording in the agent's prompt, the tool descriptions or the hands' instructions. A rule earns its place by the evals, and stays general.
- Synchronous code: threads where needed, no async runtime. Tools, gates, the chat model and the person are `Send + Sync`: a turn's calls and the agents run on threads.
- The release binary stays small (about 2.6 MB). No browser engine, and no dependency that is not already in the tree unless it earns its size; std first. Check `ls -la target/release/llx` after adding one. CI fails above 3 MiB.
- The browser window never takes keyboard focus and never stops rendering. An action is settled when the DOM has been quiet for 100 ms (at most 700 ms), and when a navigation it started has a parsed document.
- Exit codes (`apps/cli/src/exit.rs`) and the `--json` events (`apps/cli/src/output/json.rs`, `SCHEMA`) are a public contract. Change them only with a schema bump and a CHANGELOG entry.

Code:

- Unit tests live beside the code, never inside it: `x.rs` ends with `#[cfg(test)] #[path = "x_tests.rs"] mod tests;` (checked by the architecture test). Fake the ports (a scripted `Chat`, a `Gate`, a `Decider`, a `View`) instead of calling a network.
- Every file holds at most 300 lines of working code (ARCHITECTURE.md S1, checked by `budget.rs`): a file about to pass it is split by what it does first. rustfmt defaults. Comments explain why.
- The workspace's lints are hard (`Cargo.toml`, `clippy.toml`; ARCHITECTURE.md S4): no `unwrap`, `expect`, `panic!` or `unreachable!` in working code, no prints outside the output, no `process::exit` but in `main.rs`, no `Rc`/`Cell`/`RefCell`, functions at most 100 lines, `pub` only across crates. Never turn a lint off in `Cargo.toml` or for a whole module to get a change through: handle the case; an `allow` on the smallest item, with a `reason`, is the exception. `unsafe` only in `tools` and the apps (S5).
- The project stays under 50,000 lines of working code (`apps/cli/tests/suite/repo/budget.rs`; build scripts count; tests, comments and blanks do not). This is the maximum. New code is paid for first by cutting or simplifying existing code. If a change still cannot fit, stop and ask the owner before going over: say how many lines it needs, what was already cut, why the rest cannot be, and the alternatives. Never raise the limit on your own.
- English only, in code, comments and docs.

## Tests first (TDD)

Every change is red, green, refactor:

- A bug: first a test that reproduces it, at the layer where a person meets it, run and seen failing for the right reason; then the fix; then the test passes. The red run is the proof the test can fail: never break working code to get one.
- A feature or a change of behaviour: its tests first, written from what a person does and sees (keys pressed, the screen, what the model was sent), then the code.
- Refactor only on green. A test that cannot fail, or that restates a text or a constant, is deleted, not kept.

The layers:

- unit tests beside the code (`x_tests.rs`), with the ports faked;
- screen tests in the crate: the `App` driven by keys and events and drawn into a `vt100` screen (`apps/cli/src/tui/test_support.rs`), asserting what the screen shows, never the app's fields;
- one integration binary, `apps/cli/tests/all.rs` (`common/` for the harness, `suite/` for the tests): the real binary against a scripted model server that keeps every request (assert what the model received), and pty flows in `suite/tui/` that press keys and read the screen as a person does.

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
cargo test --workspace            # unit tests, apps/cli/tests/all.rs (the binary, the layer rules), packages/tools/tests/browser.rs (a real headless window)
cargo nextest run --workspace     # the same tests under cargo-nextest
scripts/coverage.sh               # line coverage with cargo-llvm-cov; fails under 85% (raised with coverage, never lowered)
cargo build --release && ls -la target/release/llx
python3 evals/run.py --repeat 3   # live, paid; run for agent, prompt, tool, loop or probe changes
python3 evals/run.py --only code  # the coding tasks alone
```

`packages/tools/tests/browser.rs` has its own `main`, because the event loop needs the main thread. `packages/tools/tests/sessions.rs` runs `kill_all_now` in a process of its own, because the table it empties is shared by every terminal session in the process.

- For every driver or probe fix, add a fixture under `packages/tools/tests/fixtures/` and an assertion in that file. Confirm the assertion fails without the fix.
- `DECISION_LOOP_DEBUG=1` prints each decision's state and answers.
- `BROWSER_DEBUG=1` prints action timings and navigation events.
- `llx -v` shows each tool result and model turn in the trace.
