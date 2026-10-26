# llx

The agent runtime other agents build on: a window of browser pages and terminals any agent can work in, helpers on any model, decisions for scripts and CI, and a minimal agent for the terminal, in one native binary. This package
installs it for your platform (macOS arm64 and x64, Linux x64).

```bash
npm install -g @localixai/llx     # then: llx
llx window                        # from another agent's shell: how to call the window
```

Another agent (Claude Code, Codex, a script) calls llx from its shell: `llx window` for tabs of
the window the person watches, `llx exec` for a helper on any model, `llx decide` for a quick
choice.

On Linux the package holds two builds: the full one, which links WebKitGTK 4.1 for the browser
window, and a headless one with no browser (terminal tabs only), which runs where WebKitGTK is not
installed, such as CI runners and servers. `llx` starts the full one only where
`libwebkit2gtk-4.1.so.0` is found; `LLX_HEADLESS=1` always starts the headless one, and
`LLX_HEADLESS=0` the full one.

Docs: <https://github.com/rsaryev/llx>
