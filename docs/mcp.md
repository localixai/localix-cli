# llx as an MCP server

`llx mcp` serves llx's `window` to another agent (any client that speaks MCP over stdio): native browser pages and terminal sessions as tabs of one window you watch, grouped by project, with the same actions, kinds and snapshots the llx agent has.

The other agent's model does the thinking, so no chat model is used. A `goal` on a tab is carried out by llx's decision model, which needs an OpenRouter key (`OPENROUTER_API_KEY`, or one saved with `/login`). Without one, everything but `goal` works.

## Connect

Any MCP client takes a server whose command is `llx` with the argument `mcp` (install it with `npm install -g @localixai/llx`):

```json
{
  "mcpServers": {
    "llx": { "command": "llx", "args": ["mcp"] }
  }
}
```

With nothing installed, the command is `npx` with the arguments `-y @localixai/llx mcp`: `npx` fetches the llx binary for your platform.

The `goal` of a tab needs an OpenRouter key: save one with `/login` in `llx`, or set `OPENROUTER_API_KEY` in the server's environment. Without one, everything but `goal` works.

Wrappers for particular agents live outside this repository; `llx mcp` is the interface they use.

## How it runs

- It runs in the daemon (started if none runs), so its tabs share the one window with your llx sessions, under the project the agent started it in; its terminals run there, with the agent's environment.
- The other agent approves each call (its own permission prompt): llx asks nothing more for them.
- A cancelled call is stopped. When the agent closes the server, its terminals end and the browser tabs it left open close.
- Comments the person sends from its tabs come back with the agent's next `window` call, after that call's report, under `### What the person sent from the window`. A wrapper can ask for them with a quiet `list` to have them arrive without the agent calling.
- Unix only. The headless Linux build has no browser, so its `window` has terminal tabs only.

The rules behind it are W9 and K7 in [ARCHITECTURE.md](../ARCHITECTURE.md).
