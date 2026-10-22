# Changelog

## 0.1.0

First release.

- **Goal in, answer out.** `lxsurf "<goal>"` opens a native browser window (the system WebView), reaches the goal and prints the answer. A chat model plans and checks; the decision model Jev clicks and types.
- **Tests and scripts.** `lxsurf check` prints PASS or FAIL; `--expect` statements are judged on the final page; `--json` and `--trace` for machines; stable exit codes 0, 1, 2, 3, 130.
- **Runs anywhere quietly.** Visible in a terminal, invisible under CI or when piped, or always with `LXSURF_HEADLESS=1`; never takes keyboard focus; a fresh private session every run.
- **OpenRouter.** `lxsurf login` signs in (with the Localix sign-in page) and saves a key, or use `OPENROUTER_API_KEY`. `--model` and `--decision-model` pick any OpenRouter chat model and System One model; `lxsurf models` lists them; `lxsurf doctor` checks everything.
- **Structured output.** `--schema` returns the result as JSON matching a JSON Schema, alone on stdout for pipelines; checked before it is printed.
- **Viewports.** `--viewport desktop|tablet|mobile|WxH`, with iOS Safari's identity for tablet and mobile; the planner can switch size mid-run.
- **What it spent.** Every run ends with time per part (planner, decision model, browser) and each model's calls, tokens (cached included) and cost; the planner's conversation only grows, so every turn reads its prefix from the prompt cache; `--json` carries the same under `usage`.
- **Keys.** The planner can press key sequences (arrow keys for a game, Escape, shortcuts) with the `key`, `code` and `keyCode` games read.
- **Page scripts.** The planner can run JavaScript in the page to extract data or operate what clicking cannot, also on sites whose Content Security Policy forbids `eval`.
- **Date pickers.** Calendar days carry their date (`15 €67 (2026-11-15)`), so a two-month picker is unambiguous; filled form fields stay visible to the planner while a modal covers them; flight searches for a date, a round trip or the cheapest day of a month work on Google Flights.
- **Guides for agents.** `lxsurf skills get core|testing`, a skill stub for `npx skills add`, and a Claude Code plugin manifest.
