# ARCHITECTURE.md

The architecture of llx, stated as rules. AGENTS.md says what each file is; this says where a
thing goes and what may know what. When a change does not fit these rules, the change is wrong,
not the rules: fix the design, or change this file first and say why in the commit.

## The shape

```
apps/cli                 composes everything; the only place that knows every package
  ├─ packages/agent      the loop, the conversation, history strategies       (contracts only)
  ├─ packages/decision-loop  the browser's hands                              (contracts only)
  ├─ packages/tools      read, edit, bash, browser, ask                       (contracts only)
  ├─ packages/session    the session store                                    (contracts only)
  └─ packages/providers/ the models' providers, and what is known of models   (contracts + kit)
       ├─ kit            HTTP, OAuth, loopback, files, SSE: shared, no port   (nothing)
       ├─ models         the model catalog: models.dev's data, typed           (contracts + kit)
       ├─ openai         ChatGPT sign-in, Responses API                       (contracts + kit)
       └─ openrouter     OpenRouter and every OpenAI-compatible endpoint      (contracts + kit)
packages/contracts       ports only: no logic, no technology                  (nothing)
```

Dependencies point inward only. `apps/cli/tests/suite/repo/architecture.rs` checks the edges; a new edge
is a design change, never a convenience.

## What goes where

| A thing | Goes in | Never in |
|---|---|---|
| A port (a trait the agent or a tool speaks) | `contracts` | anywhere else |
| Model facts: names, windows, levels, prices, dates, which providers exist | `providers/models` | the app, a provider's own table, the agent |
| How to call a provider (wire, auth, retries) | its provider package | the app |
| Which provider a choice uses, keys found, a chat built | `apps/cli/src/providers.rs` | elsewhere in the app |
| How a conversation is prepared for a request (limits, elision) | `agent/src/history.rs`, as a `Strategy` | the app, a provider |
| A tool's behaviour | its module in `tools` | the agent (which names no tool) |
| Sessions, approvals, the person, modes, the interface, agents, jobs | the app | a package |
| Data files (JSON, TOML) | the package that owns their meaning | `apps/cli/src` |

## Rules that were broken once, and must not be again

1. **One source of truth for models.** Every model fact comes from `providers/models`. No
   provider keeps its own list; no app code hard-codes a model, a window or a level. A default
   model is a policy the app states by id, and it must exist in the catalog.
2. **The app composes, it does not know.** No catalog data, wire format or provider rule lives
   in `apps/cli`. If the app needs a fact, a package exposes it.
3. **The agent names no tool and no provider** (L5). Generic signals only: a call's `path`
   argument, a tool's `parallel()`, an `Inbox`, a `Strategy`.
4. **Strategies, not special cases.** Anything that may be done another way later is a trait
   with one implementation chosen by the app: history preparation (`history::Strategy`, its
   `Pipeline` of `Stage`s), the inbox (`contracts::inbox::Inbox`), the gate, the person.
5. **Thread-safe ports.** `Tool`, `Chat`, `Gate`, `Person`, `Hands` are `Send + Sync`. State
   shared across threads is `Mutex`/atomics, never `Cell`/`RefCell`. Threads that borrow live in
   a `thread::scope` that a guard ends even on panic.
6. **The prompt cache is a constraint, not an afterthought.** Nothing rewrites an earlier message
   unless a budget forces it, and then in one pass with headroom; what was elided stays elided.
   Settings that change between tasks reach the model as a note at the end, not as an edit.
7. **Untrusted content stays data.** Tool output, agents' answers, monitor lines are tagged
   (`<agent_result>`, `<monitor>`) and never become instructions.
8. **One person, one question at a time.** Every approval and `ask` goes through
   `gate::one_question`, whichever thread asks.

## Adding things

- **A provider:** an entry in the catalog (`providers/models`, from models.dev), a wire in a
  provider package if none fits (most fit OpenAI-compatible), one arm in `providers.rs`.
- **A model:** nothing in llx: it arrives with the catalog's refresh.
- **A tool capability:** an `action` of an existing tool, or the shell. A new tool is a
  decision recorded here.
- **A history stage:** a `Stage` in `agent/src/history.rs`, placed in `Pipeline::default` by its
  value; a test in `history_tests.rs`, and the long-dialogue simulation still passing.

## Budgets

- 25,000 lines of working code, build scripts included (`apps/cli/tests/suite/repo/budget.rs`), the maximum. New code is paid for by
  cuts first; going over needs the owner's approval, asked with the numbers and the alternatives
  (AGENTS.md).
- The release binary under 3 MiB (CI; raised from 2.5 MiB by the owner for the terminal window and the windows' look); a bundled data file counts.
- New files under 300 lines; tests beside the code (`x_tests.rs`).
