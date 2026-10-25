# Tools

llx has seven tools. The set is deliberately minimal: a new capability goes into one of them (as
an `action`, or a kind of `window` tab), or is left to the shell.

| Tool | What it is for |
| --- | --- |
| [`read`](#read) | Files and directories |
| [`edit`](#edit) | Exact replacement, or a new file |
| [`bash`](#bash) | Commands that run to their end |
| [`window`](#window) | Browser pages and terminal sessions, as tabs of one window |
| [`ask`](#ask) | A question to you |
| [`agents`](#agents) | Other agents, in parallel |
| [`jobs`](#jobs) | Monitors and schedules |

## read

A file's lines, numbered, up to 2000 lines and about 20 KB at a time (`offset`, `limit`), or a directory's entries. Long lines are cut, binary files and anything but regular files (a FIFO, a device) refused. It reads anywhere (a dependency's source, a system header) except llx's own key and sessions.

## edit

- Exact replacement: `old_string` must occur once, unless `replace_all` is set. An empty `old_string` creates a file.
- Writes are atomic (a new temporary file, then a rename) and keep CRLF line endings and permissions; a read-only file is refused, and a file with other hard links is written in place.
- It asks the gate, refuses to write when the file changed while the question was open, and changes only files inside the workspace.

## bash

- `sh -c` in the workspace or a `workdir`, with stdin closed. On Unix it runs in its own session, with no terminal, so a program that prompts fails at once; git and pagers are told not to prompt or page.
- A timeout (120 s by default, at most 600 s) or Ctrl-C kills it with everything it started.
- Each stream keeps its first and last 5,000 bytes. A non-zero exit is a normal result. It asks the gate.
- The key variables of the provider in use (as the catalog names them, such as `DEEPSEEK_API_KEY`) and `OPENROUTER_API_KEY` are left out of a command's environment; other providers' variables stay, so a project's own tests can use them; a provider chosen later with `/login` has its variables left out too, from the next command on. That is no barrier, since a command can still read llx's own environment (`ps -E`, `/proc/$PPID/environ`) and the saved key file.
- A program that must ask (a REPL, an installer, another agent's interface) runs in a terminal tab of `window`.

## window

Everything the agent opens, as tabs of one window, each kind a strategy behind one interface. Terminal sessions run on between tasks, at most 4.

| Action | What it does |
| --- | --- |
| `open` | One tab, or several at once with `tabs`: `{"kind": "browser", "url": …}`, `{"kind": "terminal", "command": …}`. Returns each tab's id (`w1`, `t2`) and its snapshot: a page's outline; a terminal's transcript, or its screen when the program draws over its output. |
| `interact` | Acts in a tab, with a short `goal` and the exact `values` it may type, carried out by the decision model on any kind of tab through its view (a page's controls; a terminal's input line and keys), or with the kind's own fields (a page's `keys`, `script`, `read`; a terminal's `input`, `keys`, `wait_for`). Returns the snapshot after. |
| `screenshot` | Pictures a tab. |
| `close` | Ends one or several (`tabs`). |
| `list`, `show`, `resize` | Where you watch the window: `list` names every tab, `show` puts one in front of you and returns its snapshot, `resize` sizes the window. |

A goal, an address, keys, a script, starting a program and typing into it are approved first.

### Browser tabs

A native WebView.

- The agent opens an http(s) address (one with no scheme opens over http on this machine, `localhost`, `127.*`, `0.0.0.0` or `[::1]`, and over https elsewhere), or hands a short task with the exact values to type to the decision model, which clicks, types, selects and scrolls one step at a time.
- It can also press keys, run a script in the page, read the whole text, change the page size, take a screenshot, or close the window (the next call opens a fresh one).
- Up to 8 tabs in the one window: `tab: "new"` opens an address in a new tab, `tab: 2` switches to tab 2, `close_tab: 2` closes it, and a link with `target=_blank` or a `window.open` opens a tab instead of being lost. Every other part of a call acts on the tab shown; the list of tabs comes back only when it changed.
- The strip along the top has `‹` and `›` (back and forward in the page shown) and, at the right, a clock icon: the pages the tabs have shown, newest first, as a list over the window; a click on one opens it in a new tab. With the tabs of more than one project (several sessions share the window), a row of the projects goes above them, and the tabs under it are the shown project's.
- Tabs out of sight keep running and drawing (they stand beside the window, not hidden), and on macOS they share one cookie store, as in a browser.
- Each call returns what happened and the page as an outline. A screenshot (`screenshot: true`, cropped to an element with a CSS `selector`) is the WebView's own picture, canvas and WebGL included, sent to the model as a JPEG of at most 1280 pixels on its long side, after the call's result; only the newest three stay in the conversation. Screenshots work on macOS; elsewhere the call says they are not supported.
- It asks the gate before anything a page sees; a screenshot and a close do not ask.

## ask

One to three questions to you, in a session only (not in `exec`). Each is shown with its options to pick with the arrows or a number (a question that allows several answers toggles them with Space, then Enter), and a last one to type your own answer; Esc skips it, and Ctrl-C stops the task. It asks no gate: it changes nothing.

## agents

Starts other agents and works with them, in parallel:

- `spawn` (a task, and a short name) starts one on a thread of its own and returns at once; `send` tells one more (read at its next step while it works, or a new task once it is done); `wait` (some or all, `timeout_ms`, 30 s by default) returns the answers that came; `list` shows them; `close` ends one (at most 6 open).
- An agent's answer, its last message, reaches the main agent by itself at its next step as an `<agent_result>` message; a task does not end while its agents work.
- An agent is a session of its own whose parent is the main one: its conversation opens with the same context, it has the same tools but `agents` and `ask` (one level, and nobody to answer it), and asks the same gate, its approvals marked `[agent name]` and asked one at a time. It sees nothing of the main conversation.
- It stops with the task (Ctrl-C, the time limit) or alone (`x` on its row), and what it spends counts in the task's turns and cost (and in `--json`'s `turn` events and `usage`).
- Its tasks are saved as a session that names its parent; `llx sessions` lists the main ones with how many agents each started, and `resume` never picks an agent's.

## jobs

Work that comes back later.

- `monitor` runs a command in the background (approved like a command, in its own process group, without the key variables `bash` leaves out): each batch of lines it prints reaches the main agent as a `<monitor>` message, at its next step while a task runs, else as a task of its own. It ends when the command exits, at its deadline (5 min by default, 30 at most; 10 in `exec`, where the task waits for it) or with `stop`, and the agent is told.
- `schedule` gives a `prompt` back as a task of its own at a time: `in` (`45m`, `2h`) or a 5-field `cron` in local time (`*/10 * * * *`, `0 9 * * 1-5`; both day fields restricted match either), once or `recurring` (for 7 days at most). It runs between tasks, behind what you queued, while the session is open (not in `exec`; not restored by `resume`; no jitter).
- At most 8 monitors and 50 jobs. `list`, `stop`.

## From another agent

`llx mcp` serves `window` to other agents; see [MCP](mcp.md).
