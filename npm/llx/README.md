# llx

An ultrafast, lightweight coding agent for the terminal, in one native binary. This package
installs it for your platform (macOS arm64 and x64, Linux x64).

```bash
npm install -g @rsaryev/llx     # then: llx
npx -y @rsaryev/llx mcp         # no install: serve llx's window to another agent over MCP
```

For Claude Code or Codex:

```bash
claude mcp add llx -- npx -y @rsaryev/llx mcp
codex mcp add llx -- npx -y @rsaryev/llx mcp
```

Docs: <https://github.com/rsaryev/llx>
