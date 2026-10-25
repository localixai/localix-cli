# Modes, approvals and trust

What llx asks before it acts, how to stop it, and what it does and does not protect.

- [Modes](#modes)
- [Approvals](#approvals)
- [Stopping and quitting](#stopping-and-quitting)
- [Trust boundaries](#trust-boundaries)

## Modes

`read` never asks. `edit`, `bash` and `window` ask before they act, through one gate (`window` when a call opens an address, hands over a goal, presses keys, types into a terminal or runs a script; reading a tab does not ask). The mode decides what the gate answers, and whether `ask` gets an answer:

| Mode | What happens |
| --- | --- |
| `default` | Each command, file change and browser action is asked at the terminal. With no terminal, they are refused, and the agent is told the person must rerun with `--yes` or `--mode bypass`. Reading still works. |
| `plan` | The agent investigates and answers with a plan: a title, a summary, the changes file by file, the tests, its assumptions. File changes are refused without asking. Each command, each start of a terminal session and each text typed into one, and each browser action is asked, so it can look but you decide what runs; plan keeps its own `a` answers, apart from the default mode's, and `a` for `bash` covers its terminal sessions as it covers its commands. With no terminal, commands and browser actions are refused too, and the plan comes from reading files. |
| `bypass` (`-y`) | Everything is allowed without asking. `ask` still reaches you. |
| `autonomous` | Everything is allowed without asking, and nobody answers questions: `ask` fails at once and the agent goes on with its best assumption. In `exec`, where `ask` is not offered, it is the same as `bypass`. |

- The mode is `--mode` (or `-y`), else `default`, for a session and for `exec` alike.
- In a session, `/mode <name>` switches it for the next question, `/plan` enters plan mode, and Shift+Tab cycles default → plan → bypass → autonomous → default without Enter (queued until the current task ends). The current mode and `shift+tab to cycle` are always shown under the prompt.
- The agent is told of a change before its next task, in a `<mode_change>` note.
- When a task in plan mode ends well, llx asks `Implement this plan?`: yes goes back to the mode plan mode was entered from and runs `Implement the plan.`; no stays in plan mode, where your next message can revise the plan.

## Approvals

In a session, an approval replaces the prompt with what would happen (`Run this command?`, then the command in full) and three choices: `1. Yes`, `2. Yes, and don't ask again for bash`, `3. No, and tell llx what to do differently`, which opens a line for why.

- Pick with the arrows and Enter, or a number; Esc says no. Letters pick nothing, so words typed as the box opens cannot approve or refuse.
- Line breaks show as `⏎` and control characters are escaped, so what you approve is what runs.
- A terminal session asks when it starts and before each text or key typed into it, showing exactly what will be typed; reading its output and closing it do not ask.
- Outside a session (`exec` with a terminal), the question goes to the terminal itself as one line: `y`, `n`, `n <why>` or `a`, where only the bare word approves. This works while stdin is a pipe, stdout carries JSON and stderr goes to a file.
- A refusal goes back to the model as a failed call with its reason, and it is told not to work around it.

## Stopping and quitting

- **Esc or Ctrl-C stops the task.** The command running is killed with everything it started (and the terminal session a call is waiting on; the others stay open), an open approval or question ends at once, nothing more is approved, and the task ends at its next model turn as `interrupted` (exit 130 in `exec`), with its session saved.
- **Ctrl-C closes what is open first:** a question says no; the list under the prompt or the ← list closes, and nothing more happens. While a task works, it stops the task and keeps what you were typing. With text typed and no task, it clears the text into the earlier entries (↑ brings it back). At an empty prompt it says `ctrl-c again to quit`; a second Ctrl-C within 2 s quits, any other key cancels. A second Ctrl-C while a task is stopping quits at once, killing the terminal sessions first.
- **Ctrl-D** at an empty prompt (or `/exit`) quits at once, even while a task works. Esc on an empty prompt says `esc again to rewind`.
- **Quitting while work runs** asks first (`Background work is running`, then each thing that would stop: `session · <its title>`, `shell · <its command>`, `monitor · <name>`, `schedule · <name>`):
  - "Exit and stop tasks" stops every session, waits at most 2 s for them and exits (a task whose model request is still in flight then is not recorded);
  - "Move to background and exit", offered only when the sessions run in the daemon, leaves them working there; the next `llx` in that directory comes back to them (what they did meanwhile is drawn, then any question still waiting) and you go on in the same conversation;
  - "Stay" (or Esc) closes the question.
- A terminal closed without that choice stops the work, as "Exit and stop tasks" does. SIGTERM or closing the window stops every session's task (in the daemon, closing the window stops none).
- Terminal sessions end when llx exits, and in `exec` when the task does (its answer, `--timeout`). Closing the browser window stops the task the same way; in a session the window opens again on the next task.

## Trust boundaries

- **Tool output is data, not instructions.** File contents, command output and pages are untrusted. The system prompt says so, and every page reaches the agent marked as untrusted content. This lowers the risk of prompt injection without removing it: approve commands with that in mind, and use `-y` in places where a wrong command costs little.
- **The workspace limits `edit`, not `bash`.** `edit` changes only files inside the workspace, after following symlinks. `bash` is a real shell with your permissions, and there is no sandbox: the gate is what stands between the model and your machine. A `workdir` outside the workspace is named as such in the question.
- **llx's own files are off limits** to `read` and `edit`, symlinks included: its config directory (the saved key, the ChatGPT tokens, the `/login` choice) and the sessions directory.
- **The browser's decision model only chooses** among the operations and controls a page offers, and types only the strings the agent gave it. Its output never becomes a selector, coordinates or code. Only http(s) pages load; downloads and other schemes are refused. A password field is typed into, but its value is never read back.
- **Keys and tokens:** `/login` saves an OpenRouter key in `~/.config/llx/credentials` and a ChatGPT sign-in's tokens (access, refresh, ID, and the account id) in `~/.config/llx/openai.json` (under `XDG_CONFIG_HOME` when set), mode 0600, also when an older file was wider; the choice of provider, model and level goes in `settings.json` beside them. Keys and tokens are never printed or logged; a pasted key is read with the echo off and shown only as its masked ends. The ChatGPT access token is refreshed five minutes before it lapses (and once when a request is refused), and the refreshed tokens are saved. `llx logout` removes both files. `OPENROUTER_API_KEY` wins over a saved key when set. Commands start without it, or the key variables of the provider in use (or of one chosen later with `/login`), in their environment, but a command can still read the key from llx's environment or from the saved file: what keeps it from a command is approving each one (not `--yes`, `bypass` or `autonomous`).

A terminal session never outlives its task, so a long-lived server still runs as `cmd > log 2>&1 &`. `wait_for` can match the echo of the text typed; the report says so when the text was in the input.
