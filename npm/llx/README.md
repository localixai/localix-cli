# llx

Ultrafast, lightweight agent runtime: a native window of browser pages and terminals for any agent, a decision step for scripts and CI, and a minimal agent for the terminal, in one native binary. This package
installs it for your platform (macOS arm64 and x64, Linux x64).

```bash
npm install -g @localixai/llx     # then: llx
npx -y @localixai/llx mcp         # no install: serve llx's window to another agent over MCP
```

`llx mcp` serves the window to any MCP client: a server whose command is `llx` (or `npx` with
`-y @localixai/llx mcp`) and the argument `mcp`.

On Linux the package holds two builds: the full one, which links WebKitGTK 4.1 for the browser
window, and a headless one with no browser (terminal tabs only), which runs where WebKitGTK is not
installed, such as CI runners and servers. `llx` starts the full one only where
`libwebkit2gtk-4.1.so.0` is found; `LLX_HEADLESS=1` always starts the headless one, and
`LLX_HEADLESS=0` the full one.

Docs: <https://github.com/rsaryev/llx>
