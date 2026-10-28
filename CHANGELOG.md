# Changelog

## Unreleased

- Initial release.
- OpenRouter requests carry prompt-cache marks only for the models that take them (Anthropic, Gemini, Qwen); OpenAI, DeepSeek, Grok and the rest cache on their own and are sent plain messages.
- With a ChatGPT sign-in, the model's reasoning goes back with each next request, sealed, as the backend keeps nothing: the model no longer thinks the task through again at every turn. After `/model` switched, another model's reasoning stays out.
- Four tools: `bash`, `ask`, `agents` and `jobs`. The model reads, searches and changes files in the shell, as a person at a terminal does: `cat`, `sed -n` and `rg`, a heredoc for a whole file, a short script for a part (the context names the runtimes the shell has: `python3`, `node`, `bun`). `read`, `apply_patch` and `exec` are gone, and with them the rules that named them (`--deny apply_patch` now fails as an unknown tool). `/rewind` goes back in the conversation only: the files stay as they are.
- `plan` is read-only for real: its commands run without asking in the system's sandbox (Seatbelt on macOS, Landlock and seccomp on Linux), where writing a file, opening a socket or reaching another program fails; where no sandbox can be set up, a command is refused. A mode of your own takes `"read_only": true` in `settings.json`.
- Beside Codex on the same model, every run at once, on two reference tasks: llx sends an eighth to a half of the context on both; it is faster and cheaper on GPT-6 Luna, and passes 5 of 5 building a landing page from its brief where Codex passed 3; on GPT-6.1 Terra, Codex fixed the app to its spec faster and cheaper, and the page cost llx less at the same time ([benchmarks/arena](benchmarks/arena)).
- With a ChatGPT sign-in, a task's later requests go back to the machine that holds its prompt cache (the routing token its first reply names): fewer requests read the whole conversation afresh.
- With a ChatGPT sign-in, each session and each agent is a session of its own at the backend (`session-id` and the prompt cache's key, from the conversation's first words), where all of a process's sessions shared one: their requests spread over the backend's machines instead of crowding one.
- The model starts another agent only for a large, separate part of the work, not for what a few reads and commands settle: an agent reads it all again, at a second cost.
- `evals/arena/`: one reference task run by llx and by Codex on the same model, judged by hidden tests.
