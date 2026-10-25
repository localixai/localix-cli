# llx as an MCP server

`llx mcp` serves llx's `window` to another agent (Claude Code, Codex, Cursor, anything that speaks MCP over stdio): native browser pages and terminal sessions as tabs of one window you watch, grouped by project, with the same actions, kinds and snapshots the llx agent has.

The other agent's model does the thinking, so no chat model is used. A `goal` on a tab is carried out by llx's decision model, which needs an OpenRouter key (`OPENROUTER_API_KEY`, or one saved with `/login`). Without one, everything but `goal` works.

## Connect

One command; `npx` fetches the llx binary for your platform (the npm package `@rsaryev/llx`), so nothing needs installing first:

```bash
claude mcp add llx -- npx -y @rsaryev/llx mcp        # Claude Code
codex mcp add llx -- npx -y @rsaryev/llx mcp         # Codex
```

Claude Code can also add it as a plugin, which registers the same server (the plugin is only that wrapper, so it is the same thing as the line above):

```text
/plugin marketplace add rsaryev/llx
/plugin install llx@llx
```

Any other MCP client takes a server whose command is `npx` with the arguments `-y @rsaryev/llx mcp`. If `llx` is installed (`npm install -g @rsaryev/llx`, or the install script), the command is just `llx` with the argument `mcp`, which starts a little faster.

The `goal` of a tab needs an OpenRouter key: save one with `/login` in `llx`, or pass `-e OPENROUTER_API_KEY=sk-or-...` before the `--` in `claude mcp add`. Without one, everything but `goal` works.

## How it runs

- It runs in the daemon (started if none runs), so its tabs share the one window with your llx sessions, under the project the agent started it in; its terminals run there, with the agent's environment.
- The other agent approves each call (its own permission prompt): llx asks nothing more for them.
- A cancelled call is stopped. When the agent closes the server, its terminals end and the browser tabs it left open close.
- Comments the person sends from its tabs come back with the agent's next `window` call, after that call's report, under `### What the person sent from the window`. The Claude Code plugin asks for them every two seconds with a quiet `list` and hands them to Claude as a prompt, so they arrive without the agent calling.
- Unix only.

The rules behind it are W9 and K7 in [ARCHITECTURE.md](../ARCHITECTURE.md).
