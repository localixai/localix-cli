<p align="center">
  <img src="site/public/logo.svg" width="72" height="72" alt="llx">
</p>

<h1 align="center">llx</h1>

<p align="center">
  <strong>The ultrafast browser and coding agent</strong>
</p>

<p align="center">
  A native browser, terminals and a coding agent in one binary under 4 MB. <code>llx mcp</code> gives the same window to Claude Code, Codex or any agent.
</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="docs/usage.md">Documentation</a> ·
  <a href="#use-it-from-another-agent">Use from another agent</a> ·
  <a href="https://llx.sh">llx.sh</a>
</p>

<p align="center">
  <a href="https://llx.sh">
    <img src="docs/assets/demo.gif" alt="llx at a terminal: /login picks OpenRouter and a model, Shift+Tab switches to bypass mode, then it fixes the failing median tests in stats.py and shows the diff, in 8 seconds for $0.002" width="880">
  </a>
</p>

<table>
  <tr>
    <td width="33%" valign="top">
      <strong>A fast native browser</strong><br>
      Real pages as tabs, driven by a short goal: search, book, fill a form, check a page.<br>
      <a href="docs/tools.md">Tools →</a>
    </td>
    <td width="33%" valign="top">
      <strong>Terminals in the same window</strong><br>
      Tests, servers and REPLs as tabs beside the pages, typed into and read as you would.<br>
      <a href="docs/tools.md">Tools →</a>
    </td>
    <td width="33%" valign="top">
      <strong>Visual feedback</strong><br>
      Click what is wrong, write a note, send: the agent gets the selector, source file and a picture.<br>
      <a href="docs/mcp.md">Comments →</a>
    </td>
  </tr>
  <tr>
    <td valign="top">
      <strong>For your other agents</strong><br>
      <code>llx mcp</code> serves the window to Claude Code, Codex, Cursor and any MCP client.<br>
      <a href="docs/mcp.md">MCP →</a>
    </td>
    <td valign="top">
      <strong>Any model</strong><br>
      A ChatGPT sign-in, OpenRouter, or about 200 providers with a key.<br>
      <a href="MODELS.md">Models →</a>
    </td>
    <td valign="top">
      <strong>Safe by default</strong><br>
      Reading never asks; every command, change and page action does, by mode.<br>
      <a href="docs/modes.md">Modes →</a>
    </td>
  </tr>
</table>

## Install

```bash
npm install -g @rsaryev/llx
```

Or `curl -fsSL https://raw.githubusercontent.com/rsaryev/llx/main/scripts/install.sh | sh` (macOS and Linux x86_64, checked against `SHA256SUMS`), or `cargo install --locked --path apps/cli` from a clone. The browser is the system's WebView: WKWebView on macOS, WebKitGTK (`libwebkit2gtk-4.1-0`) on Linux. Windows is not supported for now.

```bash
cd your-project
llx                                    # a session: /login first, then a task
git diff | llx -p "review this change" # one task from stdin, then exit
llx doctor                             # sign-ins, models, the WebView, the shell
```

## Use it from another agent

One command, nothing to install first (`npx` fetches llx for your platform):

```bash
claude mcp add llx -- npx -y @rsaryev/llx mcp   # Claude Code
codex mcp add llx -- npx -y @rsaryev/llx mcp    # Codex
```

Any other client takes the command `npx` with `-y @rsaryev/llx mcp` (Cursor: `~/.cursor/mcp.json`). The Claude Code plugin also brings the comments you send from the window in as a prompt: `/plugin marketplace add rsaryev/llx`, then `/plugin install llx@llx`. A `goal` on a tab needs an OpenRouter key (`/login` in llx once, or `OPENROUTER_API_KEY`); everything else works without one. See [docs/mcp.md](docs/mcp.md).

## Documentation

[Using llx](docs/usage.md) · [Tools](docs/tools.md) · [Modes and trust](docs/modes.md) · [MCP](docs/mcp.md) · [Reference](docs/reference.md) · [Daemon](docs/daemon.md) · [Architecture](ARCHITECTURE.md) · [Models](MODELS.md) · [Changelog](CHANGELOG.md)

## Contributing

Start with [CONTRIBUTING.md](CONTRIBUTING.md) and [ARCHITECTURE.md](ARCHITECTURE.md). Report vulnerabilities as [SECURITY.md](SECURITY.md) describes. llx is released under the [MIT License](LICENSE).
