# llx for Claude Code

llx extends Claude Code: Claude stays in charge and calls llx from its own shell for what it lacks or what costs too many of its own tokens. A native window of browser pages and terminals you watch, helpers on any provider and model, quick decisions. No MCP server: the plugin's skill tells Claude the commands, and Claude composes them as its task needs.

## Install

1. llx itself, on macOS or Linux:

   ```sh
   npm install -g @localixai/llx
   ```

   or `curl -fsSL https://raw.githubusercontent.com/rsaryev/llx/main/scripts/install.sh | sh`.

2. The plugin, at the prompt of Claude Code:

   ```
   /plugin install llx --marketplace rsaryev/llx
   ```

   or from a shell: `claude plugin install llx --marketplace rsaryev/llx`.

3. The first time Claude runs `llx window …`, Claude Code asks for the command: choose to allow it always, or Claude will ask at every call.

4. Optional: a model for `llx exec`, and an OpenRouter key for a `goal` on a tab and for `llx decide`. Run `llx` in a terminal once and type `/login`. `/llx doctor` in Claude says what is there. Everything else (opening pages and terminals, clicking, typing, screenshots) needs no key.

Then ask Claude for what you want ("open localhost:3000 and check the sign-up form", "summarize these logs with a cheap model"): it reads the `llx:guide` skill and calls llx.

## What the plugin adds

- **The `llx:guide` skill.** Each block, when to use it and how, with examples: `llx window` (pages and terminals as tabs of one window; a `goal` carried out by llx's fast decision model; screenshots saved to files), `llx exec` (a self-contained task done by a helper on any model, alone or several in parallel, with exactly the rules it needs), `llx decide` (a yes or no about some text in a second), and the same blocks in CI.
- **Your comments.** Comments you send from the window's tabs (Comment, then Send) arrive as a turn of their own. The plugin notices Claude's `llx window` calls and, while this project has tabs open, asks `llx window` for what you sent; it never starts llx's daemon by itself. Tabs opened with `--as NAME` are left to Claude's next call.
- **Tidy.** When the session ends, the tabs it opened close (`llx window --end`). Two sessions in one project share its tabs.

## Commands

- `/llx` (`/llx status`): the binary and its build, the daemon and its build (current or old), this project's tabs.
- `/llx end`: closes this project's tabs and ends its terminals.
- `/llx doctor`: `llx doctor`, the sign-ins and the models.
- `/llx rebuild`: for llx's own developers: `cargo install --locked --path apps/cli` in the checkout of llx this session works in. The next llx call starts the new build once the daemon is idle.

## Developing it

`claude --plugin-dir plugins/claude-code` runs Claude Code with this folder's plugin; `claude plugin validate .` (the marketplace and the plugin) and `claude plugin test plugins/claude-code` check it.
