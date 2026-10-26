# llx in other agents

llx extends the agent you already use (Claude Code, Codex, a script of yours): it does not replace it. Your agent stays in charge, thinks, edits and checks; it calls llx from its own shell for what it lacks or what costs too much. Each block is a command, so the agent composes them as its task needs, and its own permission prompt shows every call.

| Block | What your agent gets | Command |
| --- | --- | --- |
| **window** | Real browser pages and terminals as tabs of one window the person watches; a `goal` carried out by llx's fast decision model; the person's comments on a page | `llx window` |
| **helpers** | A whole task handed to an agent on any provider and model, in the background and in parallel, with an answer in your JSON shape | `llx exec` |
| **decide** | A quick, cheap choice about some text: yes or no, or one of your options | `llx decide` |
| **sessions** | The same helper again, with its conversation | `llx exec resume ID` |

## In Claude Code

The plugin `llx` teaches Claude these blocks (its skill, `llx:guide`), brings the comments you send from the window as a turn of their own, and closes the session's tabs when it ends. Install llx, then, at Claude Code's prompt:

```
/plugin install llx --marketplace rsaryev/llx
```

Allow `llx window` always when Claude Code first asks for it. `/llx` shows the binary, the daemon and this project's tabs; `/llx end` closes them. See [plugins/claude-code](../plugins/claude-code/README.md).

## window

`llx window` is one call of the `window` tool per process. Its argument is the tool's arguments as one JSON object; `llx window` alone prints the tool's whole description and its arguments as a JSON Schema.

```sh
llx window '{"action": "open", "kind": "browser", "url": "https://example.com"}'
llx window '{"action": "interact", "tab": "w1", "goal": "accept only the necessary cookies"}'
llx window '{"action": "screenshot", "tab": "w1"}'      # the picture is saved; its path is printed
llx window '{"action": "open", "kind": "terminal", "command": "python3 -q", "wait_for": ">>>"}'
llx window '{"action": "interact", "tab": "t1", "input": "2 ** 20\n", "wait_for": ">>>"}'
llx window '{"action": "list"}'
llx window --end                                          # the owner's tabs close
```

- The tabs stay between calls for their **owner**: the project (the nearest directory with a `.git`, else the current one), or `--as NAME`. They close with `llx window --end`, when the person closes them, or when the daemon stops (`llx serve stop`).
- It runs in the daemon (started if none runs), so its tabs share the one window with your llx sessions, under the owner's project; its terminals run where it was called, with the caller's environment.
- The report is printed on stdout. A screenshot is written to a file in a directory only you can read, and its path is printed: read that file to see it.
- Exit 0 when done, 1 when the call failed (the output says why), 3 when llx could not be reached or the argument is not a JSON object.
- Your agent approves each call (its permission prompt for the command): llx asks nothing more for them. Killing the process stops the call.
- A `goal` needs an OpenRouter key (`/login` in llx, or `OPENROUTER_API_KEY`); everything else works without one. Your agent's model does the rest of the thinking: no chat model is used.
- Comments the person sends from the owner's tabs (Comment, then Send, in the window's strip) come back with its next call, after that call's report, under `### What the person sent from the window`. A quiet `list` asks for them without doing anything else.
- While an owner has a tab open, the daemon is in use: a newer llx does not replace it. `llx window --end`, then the next call starts the new build.
- Unix only. The headless Linux build has no browser, so its `window` has terminal tabs only.

## Helpers on any model

`llx exec` runs one task to its end and prints the answer alone on stdout. It thinks with any provider and model of the catalog, so your agent can hand bulk work to a cheaper one and keep its own tokens for what needs it:

```sh
llx exec --provider openrouter -m deepseek/deepseek-chat "List the public functions in src/ that have no test"
llx exec --schema @answer.json "Summarize the failing CI log in ci.log"
llx exec --allow 'bash(cargo test *)' "Run the tests and say which fail and why"
llx exec --allow browser "On example.com, find the pricing page and give its plans"
```

- With no rule it only reads: commands, file changes and browser actions are refused. `--allow` and `--deny` say what it may do ([modes and rules](modes.md)).
- `--json` prints its events; the last, `run.completed`, has the answer, the session id, the cost and the time. `exec resume ID "…"` goes on with the same helper.
- Several at once: start each in the background and read their answers when they end.
- Check what a helper says before you act on it, as you would a colleague's.

## decide

`llx decide` asks llx's decision model one question about stdin and prints the option it picked, in about a second:

```sh
git diff | llx decide "Does this change touch the public API?"           # yes or no
llx decide --choices patch,minor,major "Which version bump?" < CHANGES.md
```

The rules behind it are W9 and K7 in [ARCHITECTURE.md](../ARCHITECTURE.md).
