# llx

Ultrafast, lightweight agent runtime: a native window of browser pages and terminals for any agent, a decision step for scripts and CI, and a minimal agent for the terminal, in one native binary. This package
installs it for your platform (macOS arm64 and x64, Linux x64).

```bash
npm install -g @localixai/llx     # then: llx
npx -y @localixai/llx mcp         # no install: serve llx's window to another agent over MCP
```

`llx mcp` serves the window to any MCP client: a server whose command is `llx` (or `npx` with
`-y @localixai/llx mcp`) and the argument `mcp`.

Docs: <https://github.com/rsaryev/llx>
