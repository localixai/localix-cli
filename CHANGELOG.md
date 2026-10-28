# Changelog

## Unreleased

- Initial release.
- OpenRouter requests carry prompt-cache marks only for the models that take them (Anthropic, Gemini, Qwen); OpenAI, DeepSeek, Grok and the rest cache on their own and are sent plain messages.
- With a ChatGPT sign-in, the model's reasoning goes back with each next request, sealed, as the backend keeps nothing: the model no longer thinks the task through again at every turn. After `/model` switched, another model's reasoning stays out.
- Files change by `apply_patch`, a patch as free text (`*** Begin Patch` ... `*** End Patch`: add, delete, update and move files, hunks found by their lines), held to its grammar where the provider takes one; `edit` is gone. The model reads files and runs commands through `exec`, a short JavaScript script: independent calls at once (`Promise.all`), only what it keeps sent back. Rules and modes name `apply_patch` (`--deny apply_patch`, `apply_patch(docs/**)`). On one reference task, against Codex on the same model: less context and less time on GPT-6 Luna and GPT-6.1 Terra ([benchmarks/arena](benchmarks/arena)).
- With a ChatGPT sign-in, a task's later requests go back to the machine that holds its prompt cache (the routing token its first reply names): fewer requests read the whole conversation afresh.
- The model starts another agent only for a large, separate part of the work, not for what a few reads and commands settle: an agent reads it all again, at a second cost.
- `evals/arena/`: one reference task run by llx and by Codex on the same model, judged by hidden tests.
