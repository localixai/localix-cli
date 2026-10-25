# llx

An ultrafast, lightweight coding agent for the terminal, in one native binary. This package
installs it for your platform (macOS arm64 and x64, Linux x64).

```bash
npm install -g @localixai/llx     # then: llx
npx -y @localixai/llx mcp         # no install: serve llx's window to another agent over MCP
```

For Claude Code or Codex:

```bash
claude mcp add llx -- npx -y @localixai/llx mcp
codex mcp add llx -- npx -y @localixai/llx mcp
```

Docs: <https://github.com/rsaryev/llx>
