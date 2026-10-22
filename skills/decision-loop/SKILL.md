---
name: decision-loop
description: Decision Loop, a goal-driven CLI that drives a native browser window with a decision model. Use when a task needs a real web page operated after interaction: filling and submitting forms, logging in, picking options and dates, following multi-step flows, taking a test or quiz, or checking a web app end to end in plain words (local dev servers, smoke tests, CI). Triggers include "go to this site and…", "fill out the form…", "log in and check…", "test that the signup works", "take a screenshot of…".
allowed-tools: Bash(decision-loop:*)
---

# decision-loop

Give it a goal; a decision model drives a browser window there step by step, and it exits with what the page shows at the end. Exit codes: 0 reached, 1 not reached or a check failed, 2 out of actions or time, 3 environment not ready.

This file is a discovery stub. Before using decision-loop, load the guide that matches the installed version:

```bash
decision-loop skills get core      # running goals, quoting values, JSON, sessions, screenshots, trust boundaries
decision-loop skills get testing   # end-to-end checks with `decision-loop check` and --expect, test runners, CI
```

`decision-loop skills list` shows every guide.
