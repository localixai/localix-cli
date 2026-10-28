# ARCHITECTURE.md

The architecture of llx, as binding rules. Every change follows them, strictly; AGENTS.md says
what each file is, this says where a thing goes and what may know what. When a change does not
fit, the change is wrong, not the rules: fix the design, or change this file first, in its own
commit that says why. An exception is never made in code.

Each rule has an id. Cite it in a review or a commit when a change touches it. A rule marked
_checked_ fails a test (`apps/cli/tests/suite/repo/`) when broken; a rule is checked as soon as a
test can state it, and the test comes with the rule.

Sources: matklad's ARCHITECTURE.md and "Large Rust Workspaces", rust-analyzer's architecture and
style guides, the Rust Design Patterns' anti-patterns, corrode.dev, BurntSushi on unwrap,
sans-IO (quinn-proto, Firezone), ports and adapters, the Rust API Guidelines, LSP and JSON-RPC
2.0, the clippy configurations of ruff, uv, zed, tikv and rust-analyzer, an agent server's protocol crates, app-server client, TUI boundary check and workspace lints,
and Localix (rule ids, ratchets, poke-then-pull sync).

## Bird's-eye view

llx is a model in a loop over six tools. The loop decides; it knows no technology. Tools, stores and providers are technology behind ports. One composition root wires
them for a session. A session runs in a server, and an interface shows it through a protocol,
in the same process today, over a socket once the daemon lands (`docs/daemon.md`). Nothing a
person sees is known below the interface; nothing a model or a tool does is known above the
server.

## The layers

```text
                         apps/cli        the binary: arguments, the process, which transport
                        ╱        ╲
           apps/tui (target)    apps/server (target)    composition root: sessions, lanes,
           keys, drawing        agents, jobs, modes, the gate's and the person's answers
                │                 │
          packages/client        │        packages/agent                  the core
          (target)               │        packages/tools, session, providers/*   adapters
                │                 │
          packages/protocol (target)        packages/contracts, queue, providers/kit
          wire types only                   vocabulary: ports, primitives
```

Today `apps/cli` is all three apps in one crate; the daemon's steps split it (D1-D5 below), and
every new file already goes where its target crate will be.

| Layer | Crates | Knows |
|---|---|---|
| Vocabulary | `contracts` (ports, no logic), `queue`, `providers/kit` | no workspace crate |
| Wire | `protocol` | no workspace crate: serde only |
| Core | `agent` | `contracts` |
| Adapters | `tools`, `session` | `contracts` |
| Adapters | `providers/{models,openai,openrouter}` | `contracts`, `kit` |
| Client | `client` (in process; the socket in D3) | `protocol` |
| Server | `apps/server` (target) | every package but `client` |
| Interface | `apps/tui` (target) | `client`, `protocol` |
| Binary | `apps/cli` | `server`, `tui`, `client` |

## Dependency rules

- **L1** _checked_: `contracts` depends on no workspace crate.
- **L2** _checked_: `agent` depends only on `contracts`.
- **L3** _checked_: tools and stores depend only on `contracts`; primitives (`queue`) and the
  provider kit on nothing; providers on `contracts` and the kit. A tool knows nothing of the
  agent, the models or the other tools; a provider nothing of tools or other providers.
- **L4** _checked_: only apps compose packages. Nothing depends on an app, but the binary on the
  server and the interface.
- **L5** _checked_: `contracts` and `agent` name no tool or provider (no browser, page, URL,
  OpenRouter, OpenAI, model name). The agent learns a tool's world from `ToolSpec::description`,
  the run's place from the app's context.
- **L6** _checked_: `protocol` depends on no workspace crate. Its types are wire
  DTOs; domain types never derive its serde shapes, and it never re-exports one.
- **L7** _checked_ on the sources (and with the crate, once it is one): the interface (the
  terminal's `tui`) depends only on `client` and `protocol`: never on
  the agent, tools, providers, the session store or the server, nor on the app's other modules.
  What it needs to show, the protocol carries.
- **L8** _checked_ for the client: the server is the only crate that serves the protocol and
  the only one that knows every package; the client knows only the protocol, and is the only
  one that knows the socket.
- **L9**: a new edge between crates is a design change, recorded here first. A new `pub` item is
  an API change: visibility is the tightest that compiles (`pub(super)`, `pub(crate)`), and
  `pub use` across crates is not used to reach past a layer.
- **L10**: no `utils` or `common` crate, ever; a shared thing is named for what it is (`kit`,
  `queue`) and placed at the layer of what it knows.

## Core rules (the agent)

- **C1** _checked_ on the sources: the core decides, the shell acts. Effects leave the core
  through a port (`Chat`, `Tool`, `Gate`, `Person`, `Inbox`, `History`); the core opens
  no file, socket or process and reads no environment variable.
- **C2**: time and limits come in as values (a budget, a deadline, a strategy); a test sets them.
- **C3**: strategies, not special cases. What may be done another way is a trait with an
  implementation the app chooses: `history::Strategy` and its `Stage`s, the inbox, the gate, the
  person.
- **C4**: the prompt cache is a constraint. Nothing rewrites an earlier message unless a budget
  forces it, then in one pass with headroom; what was elided stays elided. Settings that change
  between tasks reach the model as a note at the end.
- **C5**: untrusted content stays data. Tool output, agents' answers, monitor lines are
  tagged (`<agent_result>`, `<monitor>`) where the agent reads them, never instructions.
- **C6**: (retired.)
- **C7**: no task- or site-specific wording in the prompt or the tool descriptions. A rule earns its place by the evals and stays general.

- **C8**: callers decide, functions act: branches go up to the caller, loops down into
  functions that take the batch.

## Port rules (contracts)

- **P1**: a port is a narrow trait, owned by the side that calls it, `Send + Sync`.
- **P2**: a port's error is its own enum, shaped by what the caller does with it
  (`ToolError { Failed, Fatal }`: tell the model, or end the run). No adapter's error type
  (`io`, HTTP, serde) crosses a port; the adapter translates it.
- **P3**: a value with an invariant keeps its fields private and checks them in its constructor
  (`Selection`); a value any field combination of which is valid has public fields. A struct
  never mixes a bare `pub` field with private ones (_checked_): another crate reads a private
  field through a method, and inside a crate a field says how far it reaches.
- **P4**: an option is an enum, not a `bool` or an `Option` argument, where a call site would
  otherwise read `f(true, None)`; a struct of several flags is a state enum (_checked_ for
  parameters and structs).
- **P5**: a port never returns `Result<_, String>`: its error is its enum (P2). (`History` and
  the session store still do: debt, paid when they are next touched.)
- **P6**: `map_err` keeps the cause (a variant's field, or the source); rewording an error never
  drops it (_checked_: `map_err_ignore`).
- **P7**: a crate does not re-export another crate's types, and never a glob (`pub use x::*`).

## Adapter rules (tools, stores, providers)

- **A1**: a tool asks the `Gate` before an effect outside the conversation, with a one-line
  summary that leaves nothing out. Only the app answers the gate; a tool never approves itself.
- **A2**: a tool asks the person only through `Person`, answered only by the app.
- **A3**: paths go through `Workspace` (`readable`, `writable`); a path the model wrote is never
  opened without it.
- **A4**: one source of truth for models: every model fact comes from `providers/models`. No
  provider keeps its own list; no app code hard-codes a model, window or level. A default is a
  policy the app states by a catalog id.
- **A5**: how to call a provider (wire, auth, retries) lives in its package; which provider a
  choice uses, in `apps/cli/src/providers.rs` (the server's, later) only.
- **A6**: a data file (JSON, TOML) lives in the package that owns its meaning.
- **A7**: a heavy dependency stays in the one adapter that needs it (TLS in the kit); std first, and the release binary under 5 MiB.
- **A8**, **A9**: (retired.)

## Composition rules (the apps)

- **K1**: the app composes, it does not know. No catalog data, wire format or provider rule in
  an app; if it needs a fact, a package exposes it.
- **K2**: sessions, approvals, the person, modes, agents and jobs are the server's; keys, the
  composer, drawing, `$EDITOR` and the clipboard the interface's.
- **K3**: one person, one question at a time: every approval and `ask` goes through
  `gate::one_question`, whichever thread asks. A person is one interface: a desk's lock
  (`Shared::asking`), or the process's own for the terminal it asks at; a question waiting at
  one terminal of the daemon (or in sessions moved to the background) never holds another's.
- **K4**: something that reaches a run from outside its calls is a kind in `arrivals.rs` (its
  policy and tag) posted to the lane's queue; the agent's inbox knows no source.
- **K5**: a mode is a named set of rules (`allow`, `deny`), and nothing else: the built-in ones
  (`default`, no rule; `plan`, `edit` denied; `bypass`, everything allowed) and the person's,
  from `<config>/llx/settings.json`, where one may replace a built-in. `--allow` and `--deny` add
  to the mode's. A deny refuses in every mode; an allow runs without asking; what no rule settles
  is asked of the person, or refused when no one is there. Rules come from the command line and
  the person's settings only, never from the repository (a pull request would allow itself). A
  mode changes only what the gate answers and the context's rules text; it never filters tools,
  and no code branches on a mode's name.
- **K6**: one place maps failures to the exit-code contract (`exit.rs`); the `--json` events and
  exit codes change only with a schema bump and a CHANGELOG entry.
- **K7**: (retired with `llx window`.)

## Protocol rules (the seam between interface and server)

- **W1**: newline-delimited JSON frames shaped as JSON-RPC 2.0: a request has an `id` and gets
  exactly one response (a cancelled one too, with its error); a notification has none.
- **W2**: methods are `resource/verb` with a singular resource (`session/open`,
  `request/answer`); payloads are `*Params`, `*Response`, `*Notification`; unions are tagged
  (`type`); ids are strings; times are `*_at` milliseconds since the epoch.
- **W3**: `initialize` comes first and once: versions and capabilities both ways. The protocol
  grows by optional fields and new methods; both sides ignore what they do not know. A breaking
  change is a new protocol version, and a build mismatch restarts an idle daemon.
- **W4**: one client interface, two transports: in process (channels) and the socket, with the
  same frames, so every in-process test covers the daemon's path.
- **W5**: a question the server asks a person belongs to the session: sent to every attached
  client, the first answer wins, `resolved` tells the rest, replayed on attach, aborted when its
  task stops.
- **W6**: events are numbered per session in a bounded ring; a client that falls behind gets a
  gap and reads the state again. A live-only event (a step under a call) is never replayed. A slow
  client is cut off, never waited for.
- **W7**: a request carries what the server needs to act on it (the session's id); no hidden
  state per connection but who is attached.
- **W8**: one request's panic or failure never ends the server: it becomes that request's error.
- **W9**: (retired.)

## Concurrency and errors

- **R1**: synchronous code: threads where needed, no async runtime.
- **R2**: state shared across threads is `Mutex` or atomics, never `Cell`/`RefCell`. A thread
  that borrows lives in a `thread::scope` a guard ends even on panic; a detached thread is owned
  by something that stops and joins it.
- **R3**: every queue and channel that a peer fills is bounded, or says what it dropped.
- **R4** _checked_ (S4): a panic is a bug, and one request's panic must not end the server
  (W8). Working code handles what can fail; where a value exists by construction, the smallest
  item expects the lint with the `reason` it cannot fail.
- **R5**: the app's edge has one error type (`Failure`); libraries return their port's enums.
- **R6**: a `MutexGuard` is never held across a send, a receive, a join, a sleep, I/O or a
  callback: copy out what is needed, drop the guard, then act (_checked_ where clippy sees it).
- **R7**: a check and the act that depends on it are one critical section or one atomic
  operation; two locks are always taken in one order, and a mutex is never locked twice on a
  thread.
- **R8**: every thread is named (_checked_) and started with `thread::Builder` (its failure an
  error, not a panic: _checked_), and is joined, or owned by what stops it.
- **R9**: wait on events, not on time: a `Condvar`, `recv_timeout`, a deadline. A `sleep` is a
  backoff, and says so where it is (_checked_).
- **R10** _checked_: a child process always has a deadline and a way to be stopped. (Debt: the
  app's short local queries, `git`, `osascript`, wait without one; they go through a runner with
  a deadline when `tools` exposes one.)

## Code rules

- **S1** _checked_: at most **300 lines of working code per file** (non-blank, non-comment;
  `budget.rs`), for every file of `src/` and every build script. A file that would go past it is
  split by what it does before the change lands, never let through. Test files do not count, and
  split by feature when they grow hard to read.
- **S2** _checked_: at most 50,000 lines of working code in all; new code is paid for by cuts
  first, and going over needs the owner's approval, asked with the numbers and the alternatives.
- **S3** _checked_: unit tests live beside the code: `x.rs` ends with
  `#[cfg(test)] #[path = "x_tests.rs"] mod tests;`. The ports are faked, never a network.
- **S4** _checked_: every crate takes the workspace's lints (`[lints] workspace = true`), and
  they are hard: denied, never turned off in `Cargo.toml`. An exception is an `expect` on the
  smallest item, with its `reason`, never an `allow`: an exception that no longer applies fails
  the build. In working code they forbid:
  - a panic: `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`, a string
    sliced inside a character, a number cut by `as` (R4);
  - output and exits outside their place: `print!`/`eprint!`, `dbg!`, `process::exit` (K6);
  - shared state that is not `Send + Sync`: `Rc`, `Cell`, `RefCell`; setting the environment (R2);
  - functions past 100 lines or past a bounded complexity (S1's rule for functions);
  - a `pub` no other crate reaches (L9), qualifications not needed, wildcard imports, an
    implicit `Arc` clone;
  - a `_` arm over our own enums (S10); a struct whose fields mix a bare `pub` with private
    ones (P3);
  - work done for nothing: text formatted apart to be appended (`write!` into it), a default
    built when it is not needed, a clone assigned over a value (`clone_from`), a stable sort of
    primitives, a value taken that is only read; a method returning a changed copy that is not
    `#[must_use]`.

  Libraries and binaries leave no dependency unused, in either build (the clippy checks).

  Tests may unwrap, panic and print (`clippy.toml`); an integration-test crate says so once, at
  its root. The `expect`s in working code are counted, and the count only falls: a new one raises
  it in `code.rs` in the same commit, where a review sees it (_checked_).
- **S5** _checked_: `unsafe` lives only where the OS is called: `tools` and the apps. The
  vocabulary, the core, the stores and the providers have none.
- **S6**: one idea per file, named for it; a file's `//!` says what it is (_checked_: every
  working file opens with one). A directory module (`shell/`, `fs/`) groups the files of
  one tool or one subsystem, its root `x.rs` beside the directory `x/`, never `x/mod.rs`
  (_checked_; the integration tests' `tests/common/mod.rs` and `tests/suite/mod.rs` stay, as
  cargo needs).
- **S7**: comments say why; rustfmt defaults; English only (_checked_).
- **S8**: TDD: a bug's test fails first at the layer a person meets it; a feature's tests are
  written from what a person does and sees. A test that cannot fail is deleted.

- **S9**: a clone has a reason: an `Arc` crossing threads, a value kept while the original goes
  on. A clone that only quiets the borrow checker means the data's owner is wrong.
- **S10** _checked_: your own enums are matched exhaustively; a `_` arm is for foreign or
  `#[non_exhaustive]` enums, and says so in its `expect`.
- **S11**: an `unsafe` block says why it is sound (`// SAFETY:`), one unsafe operation each
  (_checked_).

## Adding things

- **A provider:** an entry in the catalog (`providers/models`, from models.dev), a wire in a
  provider package if none fits, one arm in `providers.rs`.
- **A model:** nothing in llx: it arrives with the catalog's refresh.
- **A tool capability:** an `action` of an existing tool, or the shell. A new tool is a decision
  recorded here.
- **Something that reaches a run:** a kind in `arrivals.rs` and a post to the lane's queue (K4).
- **A CI integration:** the GitHub Action is `action.yml` at the repository's root, composite
  shell steps only (no Node, no MCP server, no GitHub tool): it installs the release its own tag
  names, runs `llx exec` once, and hands back the answer, the exit code and the spend. The
  prompt, the rules and what is posted stay in the person's workflow, where they can read them
  (C7, K5); the agent reaches GitHub, as anything else, with `gh` and `git` through `bash`. It
  reads the exit codes and `--json` events (K6), so a schema bump updates it in the same commit.
  Ready-made workflows live in `examples/`, each run for real before it is published.
- **A history stage:** a `Stage` in `agent/src/history.rs`, placed by its value in
  `Pipeline::default`, with a test, the long-dialogue simulation still passing.
- **A protocol method:** its `*Params`/`*Response` in `protocol`, served in the server, called
  through `client`; a test through the client only.
- **A rule:** here, with an id, and its test in the same commit when a test can state it.

## The daemon's steps (docs/daemon.md)

- **D1** (done): the seam as the protocol, in process: `protocol` and `client` exist; the
  interface speaks only through them. L6-L8 are checked.
- **D2** (done): sessions carry their place (directory, environment).
- **D3** (done): the socket and `llx serve`.
- **D4** (done): attach and detach: the ring, replayed questions, the quit question, back to
  the sessions moved to the background.
- **D5**, **D6**: (retired.)

## Budgets

- 50,000 lines of working code (S2), 300 per file (S1).
- The release binary under 5 MiB (CI: `scripts/check --only size`, which warns past 4 MB; raised
  from 3 MiB by the owner for the daemon and a release that unwinds a panic, W8); a bundled data
  file counts.
- Line coverage at least 85% (`scripts/coverage.sh`), raised with coverage, never lowered.
