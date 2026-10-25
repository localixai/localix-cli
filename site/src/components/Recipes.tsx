import { useState } from 'react';
import { Terminal } from './Shell';
import './InstallCommand.css';

/** Each scenario as llx composed with the tools a pipeline has; the outputs are from real runs. */
const RECIPES: [string, string][] = [
  [
    'Review',
    `$ gh pr diff 42 \\
    | llx exec --mode plan "Review this diff: bugs and risks only, with file:line." \\
    | gh pr comment 42 -F -
- lib.py:4  A hard-coded GitHub token is committed in source; revoke it.
- lib.py:10 split(sep) without a maximum split raises ValueError for key=a=b.
- lib.py:17 Removing the blank-line guard sends empty lines to parse.`,
  ],
  [
    'GitLab',
    `$ glab mr diff 42 --raw \\
    | llx exec --mode plan "Review this diff: bugs and risks only." \\
    | glab mr note create 42`,
  ],
  [
    'Gate',
    `$ git diff origin/main... | llx decide "Does this diff add a secret or token?" --expect no
yes
$ echo $?
1`,
  ],
  [
    'Triage',
    `$ gh pr diff 42 | llx decide "Which label fits?" --choices bug,feature,docs,chore
bug
$ gh run view 1234 --log-failed | llx decide "Flaky or a real bug?" --choices flaky,real
real`,
  ],
  [
    'Incident',
    `$ gh run view 1234 --log-failed \\
    | llx exec --mode plan --schema @incident.json "Classify this CI failure." | tee incident.out
{"kind":"infra","severity":"medium","retry":true,"owner":"platform",
 "summary":"Checkout got HTTP 503: a transient service failure, not the code."}
$ jq -e .retry incident.out && gh run rerun 1234 --failed`,
  ],
  [
    'Release',
    `$ git log v1.2.0..HEAD --format=%B | llx decide "What release is this?" --choices patch,minor,major
minor
$ git log v1.2.0..HEAD --oneline | llx exec --mode plan "Release notes, grouped by kind." \\
    | gh release create v1.3.0 --notes-file -`,
  ],
  [
    'After deploy',
    `$ llx exec -y --schema @smoke.json \\
    "Open https://staging.example.com, sign up with test@example.com, check it welcomes you."
{"ok":true,"seen":"Welcome!"}`,
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
