# llx as an MCP server

`llx mcp` serves llx's `window` to another agent (Claude Code, Codex, Cursor, anything that speaks MCP over stdio): native browser pages and terminal sessions as tabs of one window you watch, grouped by project, with the same actions, kinds and snapshots the llx agent has.

The other agent's model does the thinking, so no chat model is used. A `goal` on a tab is carried out by llx's decision model, which needs an OpenRouter key (`OPENROUTER_API_KEY`, or one saved with `/login`). Without one, everything but `goal` works.

## Connect

```bash
claude mcp add llx -- llx mcp                    # Claude Code
codex mcp add llx -- llx mcp                     # Codex
```

In Claude Code the plugin does the same (`llx` itself must be on your `PATH`):

```text
/plugin marketplace add rsaryev/llx
/plugin install llx@llx
```

Any other MCP client takes a server whose command is `llx` with the argument `mcp`.

## How it runs

- It runs in the daemon (started if none runs), so its tabs share the one window with your llx sessions, under the project the agent started it in; its terminals run there, with the agent's environment.
- The other agent approves each call (its own permission prompt): llx asks nothing more for them.
- A cancelled call is stopped. When the agent closes the server, its terminals end and the browser tabs it left open close.
- Unix only.

The rules behind it are W9 and K7 in [ARCHITECTURE.md](../ARCHITECTURE.md).
