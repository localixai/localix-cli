---
name: testing
description: Writing end-to-end checks in plain words with decision-loop check and --expect, for local dev servers, smoke tests and CI.
---

# decision-loop for tests

A check is a goal plus statements that must hold afterwards. decision-loop drives the page, then judges each statement against the final page.

```bash
decision-loop check 'Sign up with email "ada@example.com" and password "Str0ng!pass"' \
  --url http://localhost:3000/signup \
  --expect "The page welcomes ada@example.com" \
  --expect "No error message is shown"
# PASS            (exit 0)
# FAIL: <reason>  (exit 1)
```

Quote every value to type: the decision model types only quoted strings from the goal.

Exit 1 means the app failed the check; 2 means the run ran out of actions or time; 3 means the environment is not ready. Treat 2 and 3 as infrastructure problems, not test failures.

## In a test runner

Spawn the binary and assert on the exit code; keep `--json` output as the failure message.

```ts
// bun test / vitest
import { spawnSync } from 'node:child_process'

test('checkout works', () => {
  const run = spawnSync('decision-loop', ['check', 'Buy the cheapest T-shirt in size "M"', '--url', 'http://localhost:3000',
    '--expect', 'The order confirmation page is shown', '--json'], { encoding: 'utf8', timeout: 300_000 })
  expect(run.status, run.stdout).toBe(0)
})
```

```python
# pytest
import json, subprocess

def test_login():
    run = subprocess.run(["decision-loop", "check", 'Log in with username "demo" and password "demo"', "--url", "http://localhost:8000",
                          "--expect", "The dashboard is shown", "--json"], capture_output=True, text=True)
    assert run.returncode == 0, json.loads(run.stdout)["answer"]
```

## In CI

```yaml
# GitHub Actions
- run: decision-loop check "Open the pricing page and read the Pro plan price" --url "$PREVIEW_URL" --expect "The Pro plan price is shown"
  env:
    OPENROUTER_API_KEY: ${{ secrets.OPENROUTER_API_KEY }}
```

- The window is invisible under CI automatically.
- Linux needs WebKitGTK and a display: `sudo apt-get install -y libwebkit2gtk-4.1-0 xvfb` and run `xvfb-run decision-loop …`. macOS and Windows runners need nothing.
- Runs are isolated: each gets a fresh private session, so checks can run in parallel.
- `--trace trace.json` and `--screenshot final.png` (macOS) are good artifacts to upload on failure.

## Good checks

- Expectations are observable facts on the final page ("The cart shows 2 items"), not process ("the button was clicked").
- One user journey per check; several `--expect` for its outcome.
- Give the data the journey needs (emails, passwords of test accounts) in the goal; decision-loop never invents personal data or credentials.
