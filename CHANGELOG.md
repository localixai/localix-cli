# Changelog

## Unreleased

- Initial release.
- With a ChatGPT sign-in, the model's reasoning goes back with each next request, sealed, as the backend keeps nothing: the model no longer thinks the task through again at every turn. After `/model` switched, another model's reasoning stays out.
- `evals/arena/`: one reference task run by llx and by Codex on the same model, judged by hidden tests.
