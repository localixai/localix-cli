import { useState } from 'react';
import { Terminal } from './Shell';
import './InstallCommand.css';

/** A scenario: its tab, what it shows, and whether that is a workflow (`yaml`) or a shell session. */
type Recipe = { tab: string; session: string; yaml?: true };

/**
 * Each scenario as a step of a workflow on the action, or llx composed with the tools a pipeline
 * has. In a workflow, `= ` lines are what the step answered, shortened from runs in a test
 * repository; a second job, with the write token and no model key, posts it.
 */
const RECIPES: Recipe[] = [
  {
    tab: 'PR review',
    yaml: true,
    session: `# review.yml, job 1: the model, with a read-only token
- uses: rsaryev/llx@v0
  with:
    allow: bash
    deny: edit
    schema: |
      {"type":"object","properties":{"summary":…,"findings":…}}
    prompt: |
      Review pull request #$PR. Read the diff and the discussion with gh;
      confirm a problem by running something. Bugs and risks only.
= {"summary":"One real bug: a line without = crashes load().","findings":[
=  {"path":"src/config.py","line":5,"severity":"high",
=   "body":"A line without = raises an unpacking ValueError.",
=   "evidence":"python3 -c 'load(\\"verbose\\")' -> ValueError"}]}
# job 2: the write token, no model key, no code from the pull request. It posts the review inline.`,
  },
  {
    tab: '@llx',
    yaml: true,
    session: `# llx.yml: someone writes in a review thread: @llx why raise here instead of skipping the line?
- uses: rsaryev/llx@v0
  with:
    allow: |
      bash
      edit
    deny: |
      bash(git push *)
      bash(git commit *)
    schema: |
      {"type":"object","properties":{"reply":…,"changed":…,"title":…}}
    prompt: |
      $ASKER asked you for something on $KIND #$NUMBER. Read it and its thread with gh;
      check a claim by running it. Change the code only if they asked for a change.
= {"reply":"Raising is intentional: AGENTS.md says load names the bad line and never
=  skips it. A skipped typo would silently drop a setting. The tests pass.","changed":false}
# job 3 posts the reply in the thread; a change becomes a commit on the branch, or a draft PR.`,
  },
  {
    tab: 'Triage',
    session: `# triage.yml, on issues: opened. The repository's own labels are the choices
$ gh issue view 3 --json title,body --jq '.title + "\\n" + .body' \\
    | llx decide "What kind of issue is this?" \\
      --choices "bug=Something isn't working" --choices "enhancement=New feature or request" \\
      --choices "question=Further information is requested" \\
      --choices "documentation=Improvements or additions to documentation"
bug
$ gh issue edit 3 --add-label bug`,
  },
  {
    tab: 'CI failure',
    yaml: true,
    session: `# ci-failure.yml, on workflow_run: the tests failed
- uses: rsaryev/llx@v0
  with:
    allow: bash
    deny: edit
    schema: |
      {"type":"object","properties":{"cause":…,"path":…,"line":…,"fix":…,"rerun":…,"evidence":…}}
    prompt: |
      Run $RUN failed (gh run view $RUN --log-failed). Find the cause in the code.
      Say rerun only if running it again would likely pass.
= {"cause":"port() accepts 0, the test expects ValueError.","path":"src/config.py","line":24,
=  "fix":"Check 0 < number, as the docstring says.","rerun":false,
=  "evidence":"python3 -m unittest tests.test_config fails the same way on this commit"}
# job 2: one sticky comment on the pull request; it reruns only a flaky failure.`,
  },
  {
    tab: 'Describe PR',
    yaml: true,
    session: `# describe.yml: the author's text stays, llx's part is a marked block that is replaced
- uses: rsaryev/llx@v0
  with:
    allow: bash
    deny: edit
    schema: |
      {"type":"object","properties":{"summary":…,"changes":…,"risk":…,"risk_why":…,"testing":…}}
    prompt: |
      Describe pull request #$PR for a reviewer who has not read it: what it does and why,
      what could break and for whom, what the tests cover. Describe; do not review.
= {"summary":"Adds a worker pool, a cache and a sync job.","changes":["…"],
=  "risk":"high","risk_why":"Threads share the store without a lock.",
=  "testing":"Ran the tests: 11 tests, 1 failure, 3 errors."}`,
  },
  {
    tab: 'Release',
    yaml: true,
    session: `# release.yml, run by hand: the bump and the notes in one answer, so they agree
- uses: rsaryev/llx@v0
  with:
    allow: bash(git *)
    deny: edit
    schema: |
      {"type":"object","properties":{"bump":…,"notes":…}}
    prompt: |
      Read the log since the last tag. Pick the semantic version bump and write the
      release notes in markdown, grouped by kind. The bump must agree with the notes.
# job 2 drafts the release with gh; a person publishes it.`,
  },
  {
    tab: 'Audit',
    yaml: true,
    session: `# audit.yml, weekly: almost any task a person does with a shell
- uses: rsaryev/llx@v0
  with:
    allow: bash
    deny: edit
    max-turns: 80
    schema: |
      {"type":"object","properties":{"title":…,"body":…,"worth_filing":…}}
    prompt: |
      Explore this repository. Run the tests and linters you find. Look for real problems:
      dead code, outdated docs, flaky tests, risky dependencies. File only what is worth it.
# job 2 opens or updates one issue, labeled llx-audit.`,
  },
  {
    tab: 'Gate',
    session: `$ git diff origin/main... | llx decide "Does this diff add a secret or token?" --expect no
yes
$ echo $?
1`,
  },
  {
    tab: 'GitLab',
    yaml: true,
    session: `# .gitlab-ci.yml: the same two jobs. The first holds no write token.
llx-review:
  image: ubuntu:24.04
  script:
    - curl -fsSL https://raw.githubusercontent.com/rsaryev/llx/main/scripts/install.sh | sh
    - llx exec --allow bash --deny edit --schema @review.schema.json -o review.json "Review this merge request." </dev/null >/dev/null
  artifacts: { paths: [review.json] }
llx-post:
  needs: [llx-review]
  script:
    - jq -r .summary review.json > note.md
    - curl -fsS --header "PRIVATE-TOKEN: $LLX_POST_TOKEN" --data-urlencode "body@note.md" "$CI_API_V4_URL/projects/$CI_PROJECT_ID/merge_requests/$CI_MERGE_REQUEST_IID/notes"`,
  },
];

/** The scenarios as tabs over one terminal. */
export function Recipes() {
  const [shown, setShown] = useState(0);
  const recipe = RECIPES[shown] ?? RECIPES[0];
  if (!recipe) return null;
  return (
    <div className="recipes">
      <div className="install-tabs" role="tablist">
        {RECIPES.map(({ tab }, at) => (
          <button
            key={tab}
            type="button"
            role="tab"
            aria-selected={at === shown}
            className={at === shown ? 'on' : undefined}
            onClick={() => setShown(at)}
          >
            {tab}
          </button>
        ))}
      </div>
      <Terminal label={`${recipe.tab}: llx in CI`} session={recipe.session} yaml={recipe.yaml === true} />
    </div>
  );
}
