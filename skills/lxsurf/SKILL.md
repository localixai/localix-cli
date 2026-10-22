---
name: lxsurf
description: Localix Surf, a goal-driven browser CLI. Use when a task needs a real web page operated or read after interaction: searching a site, filling and submitting forms, logging in, picking options and dates, following multi-step flows, extracting what a page shows, or checking a web app end to end in plain words (local dev servers, smoke tests, CI). Triggers include "go to this site and…", "find on the web…", "fill out the form…", "log in and check…", "test that the signup works", "is the price on the page…".
allowed-tools: Bash(lxsurf:*), Bash(npx lxsurf:*)
---

# lxsurf

Give it a goal; it opens a native browser window, surfs until the goal is reached, prints the answer and exits. Exit codes: 0 reached, 1 not reached or a check failed, 2 out of turns or time, 3 environment not ready.

This file is a discovery stub. Before using lxsurf, load the guide that matches the installed version:

```bash
lxsurf skills get core      # running goals, JSON output, sessions, screenshots, models and keys, trust boundaries
lxsurf skills get testing   # end-to-end checks with `lxsurf check` and --expect, test runners, CI
```

`lxsurf skills list` shows every guide.
