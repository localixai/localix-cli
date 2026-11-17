# Installing llx

llx is one binary for macOS (Apple silicon and Intel) and Linux (x86_64 and arm64). For the short
version see the [README](../README.md#quick-start).

- [The install script](#the-install-script)
- [Updating](#updating)
- [Other ways](#other-ways)
- [Signing in](#signing-in)

## The install script

```bash
curl -fsSL https://raw.githubusercontent.com/localixai/localix-cli/main/scripts/install.sh | sh
```

It puts `llx`, and `lx`, a link to it, in `~/.local/bin` (or `LLX_INSTALL_DIR`), says so when that
directory is not on your `PATH`, and checks the archive against the release's `SHA256SUMS`.
It is [scripts/install.sh](../scripts/install.sh) as `main` has it, read before it runs if you like;
`https://llx.sh/install` serves the same file, a shorter address for it. To pin a release:

```bash
curl -fsSL https://raw.githubusercontent.com/localixai/localix-cli/main/scripts/install.sh | LLX_VERSION=0.3.0 sh
```

## Updating

llx checks for a newer stable release in the background when you open a terminal session.
It shows a note if one is available; it never downloads or installs a binary without a
command. Set `LLX_UPDATE_CHECK=0` to turn off the notice.

If you installed with the script, run `llx --update` from a shell outside the session.
It checks the downloaded archive before replacing the binary. If you used a custom
`LLX_INSTALL_DIR`, set it again for the update. On versions without `--update`, run the
install script again. If you installed with a package manager, update through that tool.

## Other ways

| Way | Command |
| :-- | :-- |
| [mise](https://mise.jdx.dev), the same archive | `mise use -g github:localixai/localix-cli` |
| [cargo-binstall](https://github.com/cargo-bins/cargo-binstall), the same archive | `cargo binstall --git https://github.com/localixai/localix-cli localix-cli` |
| Built with Rust 1.89 or later | `cargo install --locked --git https://github.com/localixai/localix-cli localix-cli` |
| An image, for a CI job that runs in one | `ghcr.io/localixai/localix-cli:<version>`, llx with `git`, `jq` and `curl` |

## Signing in

Run `llx` in a project and type `/login`: a ChatGPT sign-in in the browser, an OpenRouter key, or a
key for any other provider of the catalog. A provider's key in its variable (`OPENROUTER_API_KEY`)
works too, and is what CI uses. A model on your own machine (Ollama, LM Studio, llama.cpp's
`llama-server`) needs no sign-in, and any other host that speaks OpenAI's chat completions can be
added: [Signing in and choosing a model](usage.md#signing-in-and-choosing-a-model).

`llx doctor` checks the sign-ins, the models and the shell, and says what is missing.
