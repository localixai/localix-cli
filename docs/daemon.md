# llx as a daemon and a thin client (design, not yet built)

One `llx serve` per user owns everything that works: sessions (lanes), agents, jobs, models and
sign-ins, terminal sessions and the one window with its tabs. `llx` at a terminal is a client: it
draws, reads keys, and speaks to the daemon over a socket. Several terminals, several projects,
one window, one place where work goes on when a terminal closes.

Sources read: Codex's app-server (`codex-rs/app-server*`, its daemon and TUI client), Localix's API
server and UI (`docs/internals/architecture*`, `client-server-sync.md`, the wake queue), and llx's
own seam (`apps/cli/src/tui/event.rs`, `tui/ui.rs`, `desk.rs`).

## What we take, and from whom

| Decision | Taken from | Why |
|---|---|---|
| One daemon per user, started by the first client, found at a fixed socket path; a probe with a short timeout, else spawn | Codex (`daemon_auto_start`, 50 ms probe) | No setup step; a second terminal attaches |
| Unix socket in a `0700` directory, socket `0600`, no token; `flock` against a second daemon | Codex | Same-user only, std only |
| One client interface, two transports: in process (channels) and remote (socket) | Codex (`AppServerClient::{InProcess, Remote}`) | `exec`, tests and `--no-daemon` keep working with no socket |
| A request the daemon asks a person (approval, question, choice, a line) belongs to the session, not the client: sent to every attached client, first answer wins, `resolved` tells the others; replayed to a client that attaches; aborted when the task stops | Codex (server requests), Localix (HITL replay) | A closed terminal never loses a question; two terminals never both answer |
| The person's input goes to a queue on the daemon (the lane's queue), not a reply channel held by the UI | Localix (wake queue), llx's own `packages/queue` | The daemon owns all state; the client only shows it |
| A bounded per-session event ring with a cursor; a client that attaches replays after its cursor, and a gap means "read again" | Localix (`epoch:seq`, `transport.gap`) | Reattach without a durable log |
| Live-only events (a hands' step) are never replayed | Localix (replay tiers) | Replaying deltas corrupts what is drawn |
| A slow client is cut off, not waited for | Codex, Localix | The daemon never blocks on a terminal |
| Session settings (directory, environment, mode, model) live on the session; the client gives them when it opens one | Codex (thread params), Localix (session cwd) | A lane is a project; two clients, two projects |
| Signals never interrupt a task: Ctrl-C becomes `session/interrupt` | Codex | One daemon, many terminals |
| No idle exit: the daemon lives until stopped | Codex | Schedules and monitors outlive every terminal |

What we do not take: WebSocket framing (newline-delimited JSON is enough on a local socket, and
adds no dependency), schema generation, experimental API gating, an updater loop (a version
mismatch restarts an idle daemon instead), event sourcing.

## The protocol

Newline-delimited JSON frames, JSON-RPC shaped: a request `{id, method, params}`, a response
`{id, result | error}`, a notification `{method, params}`. Both sides send requests.

- Handshake: `initialize {version, build, client: {cwd, env, theme, tty, display, options}}` →
  `{daemon_version, sessions}`. A build mismatch: the client asks an idle daemon to stop and starts
  a new one; a busy one keeps it, and the client runs in process this time, saying so.
- Client → daemon: `session/open {target: new | saved(id), cwd, task?}`, `session/attach {id,
  cursor}`, `session/detach`, `session/input {id, text, images}` (a task, a steer, a slash
  command), `session/interrupt {id}`, `session/halt {id}`, `session/list`, `request/answer {id,
  answer}`, `window/...` none (the window is the daemon's), `daemon/stop`.
- Daemon → client notifications: today's `Event`s, each scoped to a session and numbered
  (`{session, seq, event}`): header, note, task started and ended, call, result, answer, steps,
  jobs, agents, title, context left, queue changes (what waits, what was steered).
- Daemon → client requests: `approve`, `ask`, `choose`, `choose_adjustable`, `line` (a secret
  stays hidden on the client and travels once), each with the session it belongs to; `resolved
  {request}` when one is answered or aborted.

Today's reply channels map one to one: `Ready` becomes the daemon's own input queue, `TakeSteered`
becomes the queue's steer entries, `Approve`/`Ask`/`Choose`/`Line` become daemon → client
requests. Types that cross need serde: `ToolOutput`/`ToolError` (results are sent as their
report, state elided, image dropped), `Approval`, `Question`/`Answer`, `Typed`, `JobRow` (its
`Instant` becomes milliseconds left), `meta::Update`, `picker::Adjustment`.

## Who owns what

| | Daemon | Client |
|---|---|---|
| Sessions, agents, jobs, queues, history | ✓ | |
| Models, sign-ins, the catalog refresh | ✓ | |
| Terminal sessions, the window (main thread, `tao`) | ✓ | |
| Keys, composer, drawing, raw mode | | ✓ |
| `$EDITOR`, Ctrl+V (a picture is kept in the shared pasted folder and sent as its path) | | ✓ |
| `/copy` (the daemon sends the text, the client copies it) | | ✓ |
| Terminal colours (asked once, sent at `initialize`) | | ✓ |
| `@` file list | ✓ (asked by the client) | |

A session's directory and environment become the session's (today `Shared.workspace` and the
process environment): commands run in the environment of the client that opened the session
(its `PATH`, an activated virtualenv, its keys), not the daemon's. Its hidden key variables are computed from that environment.

## Lifecycle

- `llx`: probe `$TMPDIR/llx-<uid>/<hash of the config dir>.sock` (a test home gets its own
  daemon); none → spawn `llx serve` detached (`setsid`), wait for its ready line; attach.
- `llx serve`: `flock` on `<config>/llx/daemon.lock`, bind, write a ready line, run the window's
  event loop on the main thread and the sessions on threads.
- Quitting while work runs asks, as Claude Code does:

  ```
  Background work is running
  The following will stop when you exit:
    shell · Build Electron with siso remote cache
  ❯ 1. Exit and stop tasks
    2. Move to background and exit
    3. Stay
  ```

  Only moving to the background keeps them: the sessions go on in the daemon (their tasks,
  monitors and schedules), and the next `llx` in that directory shows them. A terminal closed
  without that choice stops its sessions' work, as "Exit and stop tasks" does; the daemon itself
  lives on.
- The daemon never exits by itself: schedules and monitors must outlive every terminal. It ends
  with `llx serve stop`, or with the machine.
- `llx serve status|stop`; `--no-daemon` runs everything in process, as today; `exec` always does.

## Steps, each shipped green

1. **The seam as a protocol, in process.** Message types with serde in `packages/protocol`, the
   client interface in `packages/client` with an in-process transport over channels
   (ARCHITECTURE.md L6-L8, W1-W8); the TUI and the desk speak only through it. Reply
   channels become requests with ids and the daemon-side input queue. No behaviour changes; every
   test stays.
2. **Sessions carry their place.** Directory, environment, hidden variables and theme per
   session; `/copy` and the clipboard on the client.
3. **The socket.** `llx serve`, discovery, spawn, lock, handshake, the remote transport; the
   window host in the daemon. Pty tests get a daemon per test home.
4. **Attach and detach.** The event ring and cursors, pending requests replayed, the quit
   question (stop, move to the background, stay), `status`/`stop`, version restart.
5. **The window by project.** Tab groups in the strip per session's directory. Built: each tab
   has a group (`Browser::group`, by key; a page's own new tab takes its opener's), named by the
   app per lane (`windows::group`, the session's directory's last component): a lane's `Tabs`
   claims the tabs its `window` calls open (`Windowed::opened`), its terminals' panels are
   grouped as they open. The strip draws each group's tabs together, the group's name before
   them, only when there are several; `window list` names a tab's group only then. The tool
   knows no project. Left for D3/D4: a daemon's lanes from several directories feed it.

Estimated 2,500 to 3,500 lines; the budget has 5,000 left.
