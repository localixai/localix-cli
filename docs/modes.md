# Modes, approvals and trust

What llx asks before it acts, how to stop it, and what it does and does not protect.

- [Modes](#modes)
- [Approvals](#approvals)
- [Stopping and quitting](#stopping-and-quitting)
- [Trust boundaries](#trust-boundaries)

## Modes

`read` never asks. `edit`, `bash` and `window` ask before they act, through one gate (`window` when a call opens an address, hands over a goal, presses keys, types into a terminal or runs a script; reading a tab does not ask). What the gate answers comes from rules:

1. A **deny** rule that matches refuses, and the refusal names the rule.
2. An **allow** rule that matches runs it without asking.
3. Anything else is asked of the person at the terminal; with no one there (`exec`, CI), it is refused, and the agent is told how the person can allow it.

A **mode** is a named set of those rules, and nothing more. Three are built in:

| Mode | Rules |
| --- | --- |
| `default` | none: everything is asked, or refused with no one to ask |
| `plan` | deny `edit`: read-only, file changes refused without asking |
| `bypass` (`-y`) | allow everything |

Your own go in `modes` in `~/.config/llx/settings.json` (under `XDG_CONFIG_HOME` when set); one with the name of a built-in mode replaces it:

```json
{
  "modes": {
    "review": { "allow": ["bash(cargo test *)", "bash(git log *)"], "description": "Review, tests allowed." },
    "fix":    { "allow": ["bash(cargo *)", "edit(src/**)", "edit(tests/**)"], "deny": ["bash(git push *)"] },
    "smoke":  { "allow": ["browser"] }
  }
}
```

- `--mode NAME` chooses one, `-y` is `bypass`, else `default`; for a session and for `exec` alike. A name that is not a mode, or a mode whose rules cannot be read, stops the run before a model is called (exit 3).
- `--allow RULE` and `--deny RULE`, repeated, add to the mode's rules for that run: `llx exec --mode review --deny 'bash(cargo publish *)' …`.
- In a session, `/mode` lists the modes, `/mode <name>` switches for the next question, and Shift+Tab goes round them all without Enter (queued until the current task ends). The agent hears of a change before its next task, in a `<mode_change>` note; its context lists the rules in force.
- Rules come from the command line and your settings only, never from the repository: a pull request in CI could otherwise allow itself anything. In a container, mount your settings file read-only (`-v ./llx.json:/root/.config/llx/settings.json:ro`) and pass the key as a variable; `/login` writes to that file, so do not sign in through a read-only one.

### Rules

Rules are written as Claude Code writes them:

| Rule | Matches |
| --- | --- |
| `bash` or `bash(*)` | every command |
| `bash(cargo test)` | that command exactly |
| `bash(cargo test *)` | `cargo test`, `cargo test -q`, …, not `cargo testing` |
| `bash(* --version)` | any program's `--version` |
| `edit` | every file change |
| `edit(docs/**)` | a file under `docs/`, from the workspace root |
| `edit(*.md)` | a Markdown file at any depth |
| `browser`, `terminal`, `jobs` | every page action, every goal on a terminal tab, every monitor or schedule |

- `*` stands for any text, spaces included; the words before it limit the rule, so put it after the subcommand (`git log *`, not `git * main`).
- A command is **allowed** only when every part of it matches an allow rule: it is split at `&&`, `||`, `;`, `|`, `&` and line breaks, outside quotes. One with `$(…)`, backticks, `<(…)`, a here-document, a redirect to a file (`> /dev/null` and `2>&1` are fine), `eval`, `sh -c`, `tee` to a file or `find -exec`/`-delete` is never allowed by a pattern: it is asked, or refused with no one to ask. `timeout`, `time`, `nice`, `nohup`, `stdbuf` and a bare `xargs` are looked through; a leading `VAR=value` stops an allow.
- A command is **denied** when any part matches a deny rule, inside `$(…)` and past any `VAR=value` too.
- A rule matches the command as written: `bash(rm *)` in deny does not stop `/bin/rm` or `sh -c 'rm …'`. It is a guard on what the model usually writes, not a sandbox; for that, run llx in a container.
- Typing into a terminal session has no command to match: only a bare `bash` allows or denies it.

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
- **Keys and tokens:** `/login` saves an OpenRouter key in `~/.config/llx/credentials` and a ChatGPT sign-in's tokens (access, refresh, ID, and the account id) in `~/.config/llx/openai.json` (under `XDG_CONFIG_HOME` when set), mode 0600, also when an older file was wider; the choice of provider, model and level goes in `settings.json` beside them. Keys and tokens are never printed or logged; a pasted key is read with the echo off and shown only as its masked ends. The ChatGPT access token is refreshed five minutes before it lapses (and once when a request is refused), and the refreshed tokens are saved. `llx logout` removes both files. `OPENROUTER_API_KEY` wins over a saved key when set. Commands start without it, or the key variables of the provider in use (or of one chosen later with `/login`), in their environment, but a command can still read the key from llx's environment or from the saved file: what keeps it from a command is approving each one (not `--yes`, `bypass`, or a rule that allows it).

A terminal session never outlives its task, so a long-lived server still runs as `cmd > log 2>&1 &`. `wait_for` can match the echo of the text typed; the report says so when the text was in the input.
