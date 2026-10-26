# Examples

GitHub workflows that use llx, each run for real in a test repository. Copy a workflow into
`.github/workflows/` and its files from `llx/` into `.github/llx/`, then add the secret
`OPENROUTER_API_KEY`.

| Workflow | When | What llx does |
|---|---|---|
| [review.yml](github/review.yml) | a pull request opens or changes | explores the merged tree, runs what it needs, posts inline comments and one summary it updates |
| [triage.yml](github/triage.yml) | an issue opens | picks a kind and a priority from the repository's own labels with `llx decide` |
| [ci-failure.yml](github/ci-failure.yml) | the tests fail on a pull request | finds the cause in the code, comments, reruns only a flaky failure |
| [describe.yml](github/describe.yml) | a pull request opens | writes a summary, the risk and the testing below the author's text |
| [fix-issue.yml](github/fix-issue.yml) | a maintainer labels an issue `llx` | changes the code and tests, then a draft pull request |
| [release.yml](github/release.yml) | run by hand | picks the version bump, writes notes, drafts a release |

How they are built:

- The job that runs the model has a read-only token (`contents: read`); a separate job with write
  access posts the result. Nothing the model reads can make it push or comment.
- The agent works in a throwaway runner: `--allow bash --deny edit` lets it read and run anything
  and change no file; `fix-issue.yml` allows edits but not `git push` or `git commit`.
- `--schema` gives JSON the next job can trust; lines the model names are checked against the diff
  before they are posted.
- Values from the event go through `env:`, never into the script itself, and the prompt says the
  code, issues and comments are data, not instructions.
- On a self-hosted runner, replace `ubuntu-latest` with its labels.
