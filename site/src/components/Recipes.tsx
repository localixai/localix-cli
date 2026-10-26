import { useState } from 'react';
import { Terminal } from './Shell';
import './InstallCommand.css';

/** Each scenario as llx composed with the tools a pipeline has; the outputs are shortened from runs in a test repository. */
const RECIPES: [string, string][] = [
  [
    'PR review',
    `# .github/workflows/review.yml: the agent explores the merged tree itself, read-only
$ llx exec --allow bash --deny edit --schema @review.schema.json \\
    "REPO: $REPO  PULL REQUEST: #$PR. Review it: read the diff, the files, the discussion;
     confirm a problem by running something. Bugs and risks only."
{"verdict":"request_changes","findings":[
 {"path":"src/config.py","line":5,"severity":"high",
  "body":"A line without = raises an unpacking ValueError.",
  "evidence":"python3 -c 'load(\\"verbose\\")' -> ValueError"}]}
# a separate job with write access posts it as inline comments`,
  ],
  [
    'Triage',
    `# .github/workflows/triage.yml: on issues: opened, the repo's own labels as choices
$ gh issue view 3 --json title,body --jq '.title + "\\n" + .body' \\
    | llx decide "What kind of issue is this?" \\
      --choices "bug=Something is broken" --choices "feature=A new capability" \\
      --choices "question=Needs an answer" --choices "docs=Documentation"
bug
$ gh issue edit 3 --add-label bug`,
  ],
  [
    'CI failure',
    `# .github/workflows/ci-failure.yml: on workflow_run failed
$ llx exec --allow bash --deny edit --schema @failure.schema.json \\
    "Run $RUN failed (gh run view $RUN --log-failed). Find the cause in the code."
{"kind":"code","path":"src/config.py","line":24,"rerun":false,
 "cause":"port() accepts 0, the test expects ValueError.",
 "fix":"Check 0 < number, as the docstring says."}
# posted as one sticky comment; rerun only when the failure is flaky`,
  ],
  [
    'Reply',
    `# .github/workflows/reply.yml: someone writes @llx in a review thread
> @llx why raise here instead of skipping the line with a warning?
$ llx exec --allow bash --deny edit --schema @reply.schema.json \\
    "PULL REQUEST: #6. Answer the thread on src/config.py:16; check a claim by running it."
{"agrees":false,"reply":"Raising is intentional: AGENTS.md says load names the bad line
 and never skips it. A skipped typo would silently drop a setting. The tests pass."}
# a separate job posts it as a reply in the thread`,
  ],
  [
    'Describe PR',
    `# .github/workflows/describe.yml: the author's text stays, llx's part is replaced
$ llx exec --allow bash --deny edit --schema @describe.schema.json \\
    "Describe pull request #$PR for its reviewers: what changes, the risk, how it was tested."
{"summary":"Adds a worker pool, a cache and a sync job.",
 "risk":"high","risk_why":"Threads share the store without a lock.",
 "testing":"Ran the tests: 11 tests, 1 failure, 3 errors."}`,
  ],
  [
    'Fix an issue',
    `# .github/workflows/fix-issue.yml: a maintainer labels an issue llx
$ llx exec --allow bash --allow edit --deny 'bash(git push *)' \\
    "ISSUE: #5. Resolve it: the smallest change, a test, run the tests. Do not commit."
$ git diff > change.patch   # a separate job pushes it and opens a draft PR`,
  ],
  [
    'Release',
    `$ git log v1.2.0..HEAD --format=%B | llx decide "Which semantic version bump?" \\
    --choices "major=breaks users" --choices "minor=a feature" --choices "patch=fixes only"
minor
$ llx exec --allow 'bash(git *)' --deny edit -o notes.md "Release notes since v1.2.0."
$ gh release create v1.3.0 --draft --notes-file notes.md`,
  ],
  [
    'Gate',
    `$ git diff origin/main... | llx decide "Does this diff add a secret or token?" --expect no
yes
$ echo $?
1`,
  ],
  [
    'GitLab',
    `$ llx exec --allow bash --deny edit \\
    "Review merge request !$CI_MERGE_REQUEST_IID (glab mr view, glab mr diff). Bugs only." \\
    | glab mr note create $CI_MERGE_REQUEST_IID`,
  ],
];

/** The scenarios as tabs over one terminal. */
export function Recipes() {
  const [shown, setShown] = useState(0);
  const [name, session] = RECIPES[shown] ?? RECIPES[0] ?? ['', ''];
  return (
    <div className="recipes">
      <div className="install-tabs" role="tablist">
        {RECIPES.map(([tab], at) => (
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
      <Terminal label={`${name}: llx in a shell`} session={session} />
    </div>
  );
}
