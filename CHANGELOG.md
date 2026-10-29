# Changelog

## Unreleased

- Initial release.
- An answer that does not say the goal is reached (`RESULT: not achieved`, or no RESULT line after the task made calls) goes back to the model: work left (failing checks, missing pieces) is its next step, not a reason to stop. It stands when it comes right after one sent back, with nothing done since, or after three.
- `bash` keeps about 40,000 characters of each stream (some 10,000 tokens, as Codex does), where it kept 10,000: a test file or a build log reaches the model whole, not cut in the middle. A tool result keeps up to 90,000 characters.
- With a ChatGPT sign-in, `apply_patch` is offered again as a tool held to its grammar for the models that take one: a fetched catalog (models.dev's v2 API, which does not publish it) had turned it into a JSON function for every OpenAI model, and patches came back cut short or malformed.
- A patch whose lines are not found in the file says so with the file's nearest lines, numbered: the next patch needs no read first.
- When the conversation is trimmed for the model's limit, the text of old patches goes as other large arguments do.
- Fixed: an `--allow` or `--deny` rule with many wildcards no longer takes exponential time on a long command or path that does not match.
- Fixed: a number key pressed on a choice with no options no longer ends the interface.
- Markdown's ~~strikethrough~~ shows struck through; `/usage` says the cost once, the time on a line of its own.
- A task has no turn limit unless `--max-turns` gives one (it was 50, and cut long tasks off while they were still fixing what failed); `--timeout` bounds it. The GitHub Action's `max-turns` has no default either.
- OpenRouter requests carry prompt-cache marks only for the models that take them (Anthropic, Gemini, Qwen); OpenAI, DeepSeek, Grok and the rest cache on their own and are sent plain messages.
- With a ChatGPT sign-in, the model's reasoning goes back with each next request, sealed, as the backend keeps nothing: the model no longer thinks the task through again at every turn. After `/model` switched, another model's reasoning stays out.
- Five tools: `bash`, `apply_patch`, `ask`, `agents` and `jobs`. The model reads, searches and runs the checks in the shell, as a person at a terminal does (`cat`, `sed -n`, `rg`; the context names the runtimes the shell has: `python3`, `node`, `bun`), and changes files with `apply_patch`, a patch of free text: a custom tool held to its grammar where the ChatGPT backend takes one, a function elsewhere. In a read-only mode a patch is refused. `read` and `exec` are gone. `/rewind` goes back in the conversation only: the files stay as they are.
- `plan` is read-only for real: its commands run without asking in the system's sandbox (Seatbelt on macOS, Landlock and seccomp on Linux), where writing a file, opening a socket or reaching another program fails; where no sandbox can be set up, a command is refused. A mode of your own takes `"read_only": true` in `settings.json`.
- Beside Codex on the same model, every run at once, on two reference tasks: llx sends an eighth to a half of the context on both; it is faster and cheaper on GPT-6 Luna, and passes 5 of 5 building a landing page from its brief where Codex passed 3; on GPT-6.1 Terra, Codex fixed the app to its spec faster and cheaper, and the page cost llx less at the same time ([benchmarks/arena](benchmarks/arena)).
- With a ChatGPT sign-in, a task's later requests go back to the machine that holds its prompt cache (the routing token its first reply names): fewer requests read the whole conversation afresh.
- With a ChatGPT sign-in, each session and each agent is a session of its own at the backend (`session-id` and the prompt cache's key, from the conversation's first words), where all of a process's sessions shared one: their requests spread over the backend's machines instead of crowding one.
- The model starts another agent only for a large, separate part of the work, not for what a few reads and commands settle: an agent reads it all again, at a second cost.
- `evals/arena/`: one reference task run by llx and by Codex on the same model, judged by hidden tests.
