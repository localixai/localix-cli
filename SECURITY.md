# Security

## Reporting a vulnerability

Please report it privately through
[GitHub's security advisories](https://github.com/rsaryev/decision-loop/security/advisories/new),
not in a public issue. Say what an attacker can do, and how to reproduce it. You will get an answer
within a week; a fix goes out as a patch release with credit to you, unless you prefer otherwise.

Only the latest release is supported.

## What lxsurf can do on your machine

lxsurf is an agent: a language model decides what its tools do. Run it where you would run the
commands yourself.

- **Read** files in the working tree.
- **Edit** files, and **run shell commands** (bash) with your user's rights. Each of these goes
  through an approval gate first: the tool describes the effect in one line (the command, or the
  file and its changed lines), and it runs only when allowed. A denied effect is reported back to
  the model, which can try another way. Allowing everything up front (`--yes`) skips the gate:
  use it only in a sandbox, a container or a throwaway checkout.
- **Browse** in a native WebView window, which can submit forms and run page scripts the model
  asks for, with that window's own cookies, not your browser's.

What the agent reads (files, command output, web pages) is untrusted data. It is passed to the
model as content, never as instructions, but a model can still be misled by what it reads (prompt
injection), so the gate is the line of defence for anything that changes your machine. Typed text
and click targets come from the goal and the page's own controls; model output never becomes a
selector, coordinates or code for the browser's hands.

## Keys

The OpenRouter key comes from `OPENROUTER_API_KEY` or from `lxsurf login`, which saves it in a file
readable only by you (mode 0600 on Unix). `lxsurf logout` deletes it. Goals, page text and command
output are sent to OpenRouter and the models you choose; do not point lxsurf at secrets you would
not send there.

## Releases

Release archives are built by GitHub Actions from the tagged commit and published with a
`SHA256SUMS` file; `install.sh` and `install.ps1` refuse an archive whose checksum does not match.
