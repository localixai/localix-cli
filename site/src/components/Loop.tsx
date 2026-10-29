import { type CSSProperties, useEffect, useRef } from 'react';
import { AGENTS_DOCS, CI_DOCS, DOCS, GITHUB } from '../links';
import './Loop.css';

/** A line of a panel: its text in parts, each with a tone (the terminal's colours). */
type Tone = 'dim' | 'hi' | 'ok' | 'bad' | 'acc' | 'sel' | '';
type Line = [Tone, string][];

type Card = { title: string; text: string; lines: Line[]; link?: { href: string; label: string } };

/** The exit codes are exit.rs; the benchmark card is benchmarks/task-tracker's runs (medians); the
 *  rules card uses llx's own wording of a refusal (packages/tools/src/lib.rs). */
const CARDS: Card[] = [
  {
    title: 'Ends every task with a result',
    text: 'The agent ends a task by calling result: achieved or not, a summary, what blocked it, and data matching your JSON Schema. Text alone never ends a task. The exit code says how it ended.',
    link: { href: CI_DOCS, label: 'The CI guide' },
    lines: [
      [['dim', '0   '], ['ok', 'achieved    '], ['dim', ' the goal was reached']],
      [['dim', '1   '], ['bad', 'not_achieved'], ['dim', ' it was not, or the data missed the schema']],
      [['dim', '2   '], ['', 'exhausted    '], ['dim', ' out of turns, time or money']],
      [['dim', '3   '], ['', 'error        '], ['dim', ' no key, no network, a bad argument']],
      [['dim', '130 '], ['', 'interrupted  '], ['dim', ' Ctrl-C or SIGTERM']],
    ],
  },
  {
    title: 'A step your pipeline can branch on',
    text: 'llx exec runs one task and prints JSON events; the last one holds the status, your data and the cost. On GitHub, uses: rsaryev/llx@v0 runs the same step and returns the answer, the status and the spend.',
    lines: [
      [['dim', '$ '], ['', 'llx exec --json --mode plan --schema @ship.json \\']],
      [['', '    "Safe to ship this diff? Which version bump?" > events.jsonl']],
      [['dim', '$ '], ['', 'echo $?']],
      [['ok', '0']],
      [['dim', '$ '], ['', "jq -c 'select(.type == \"run.completed\") | {status, data}' events.jsonl"]],
      [['hi', '{"status":"achieved","data":{"bump":"patch","ship":true}}']],
    ],
  },
  {
    title: 'Three limits, the first reached ends the task',
    text: '--max-turns, --timeout (30 minutes by default) and --budget in dollars. A task that runs into one ends with exit 2, exhausted, so a pipeline can tell it from a failure.',
    lines: [
      [['dim', '$ '], ['', 'llx exec --timeout 600 --budget 0.50 --max-turns 40 \\']],
      [['', '    "Fix the failing test"']],
      [['dim', '$ '], ['', 'echo $?']],
      [['bad', '2'], ['dim', '   exhausted: out of turns, time or money before a result']],
    ],
  },
  {
    title: 'Runs under your rules',
    text: 'A mode is a set of rules: what runs without asking, what is refused in every mode. They come from your command line and settings, never from the repository. plan runs read-only in the system sandbox.',
    link: { href: `${DOCS}/modes`, label: 'Modes and trust' },
    lines: [
      [['hi', 'default'], ['dim', '  each command is asked']],
      [['hi', 'plan   '], ['dim', '  read-only, in a sandbox']],
      [['hi', 'bypass '], ['dim', '  everything runs (-y)']],
      [['dim', '$ '], ['', "llx exec --allow 'bash(cargo test *)' \\"]],
      [['', "    --deny 'bash(git push *)' \"Fix the failing test\""]],
      [['bad', '• '], ['hi', 'Ran'], ['', ' git push']],
      [['bad', '  └ not allowed: refused by the rule `bash(git push *)`']],
    ],
  },
  {
    title: 'Agents in parallel, jobs in the background',
    text: 'One task, split: each agent is a session of its own, running at once, answering back to the one that started it. Jobs watch a command or run on a schedule and report back while the work goes on.',
    lines: [
      [['acc', '• '], ['hi', 'Spawned'], ['', ' median · why the median tests fail']],
      [['acc', '↳ '], ['hi', 'Spawned'], ['', ' mean · mean() on an empty list']],
      [['ok', '● '], ['', 'Agent "median" finished · 6s']],
      [['ok', '● '], ['', 'Agent "mean" finished · 6s']],
      [['ok', '• '], ['hi', 'Ran'], ['', " python3 - <<'EOF'  "], ['dim', '# stats.py, the median']],
      [['', '• Fixed '], ['acc', 'stats.py:15'], ['', ': averages the two middle values.']],
    ],
  },
  {
    title: 'Any provider, any model',
    text: 'Pick a provider and a model with /model: a ChatGPT sign-in, OpenRouter, or a key for the provider you use. The reasoning level with ← →, saved for every run after.',
    lines: [
      [['hi', '[OpenAI 2]'], ['dim', ' · Anthropic 0 · OpenRouter 5 · xAI 0 · DeepSeek 0']],
      [['dim', 'Search: terra▏  7 matches']],
      [['sel', '❯ GPT-6.1 Terra      gpt-6.1-terra      ← low →']],
      [['', '  GPT-6.1 Terra Pro  gpt-6.1-terra-pro    medium']],
      [['dim', 'tab provider · ←→ reasoning · enter select']],
      [['dim', 'Now openai · gpt-6.1-terra · low']],
    ],
  },
  {
    title: 'Beside Codex, on the same model',
    text: "One task: build a task tracker's API on Bun from its tests, scored out of 199 (132 public tests, 66 hidden ones, the types). GPT-6 Luna at medium reasoning, every run at once; medians.",
    link: { href: `${GITHUB}/tree/main/benchmarks/task-tracker/runs`, label: 'The runs' },
    lines: [
      [['dim', '            runs   score    time     cost  finished']],
      [['hi', 'llx     '], ['hi', '     6'], ['hi', '     195'], ['', '    170s'], ['', '  $0.026'], ['', '       6/6']],
      [['', 'Codex   '], ['', '     3'], ['', '     197'], ['', '    143s'], ['', '  $0.025'], ['', '       3/3']],
    ],
  },
  {
    title: 'For other agents: decide in a second',
    text: 'Claude Code, another agent or a script can call llx from its shell. llx decide picks one of the options you give, alone on stdout; llx exec hands a task to a helper on any model.',
    link: { href: AGENTS_DOCS, label: 'llx for other agents' },
    lines: [
      [['dim', '$ '], ['', 'git diff | llx decide --choices patch,minor,major \\']],
      [['', '    "Which version bump?"']],
      [['ok', 'patch']],
      [['dim', '$ '], ['', "echo 'App crashes when the cart is empty' |"]],
      [['', '    llx decide --choices bug,feature,question "Label it"']],
      [['ok', 'bug']],
      [['dim', '$ '], ['', 'git diff | llx exec --mode plan "A one-line commit message"']],
      [['', 'Fix median calculation for even-length lists']],
    ],
  },
  {
    title: 'Five tools, one binary under 4 MB',
    text: 'bash reads, searches and runs the checks; apply_patch changes files; ask puts a question to you; agents and jobs do the rest. No plugins, no skills, no servers to run. At your terminal it is a minimal agent: sign in with /login, then give it a task.',
    link: { href: DOCS, label: 'The docs' },
    lines: [
      [['hi', 'bash        '], ['dim', ' read, search, run the checks']],
      [['hi', 'apply_patch '], ['dim', ' add, change, delete, move files']],
      [['hi', 'ask         '], ['dim', ' a question to the person']],
      [['hi', 'agents      '], ['dim', ' other agents, in parallel']],
      [['hi', 'jobs        '], ['dim', ' monitors and schedules']],
      [['acc', 'result      '], ['dim', ' how every task ends']],
    ],
  },
];

/** The steps one after another, each card held at the top while the next slides over it; a
 *  card's lines come in once it is seen. Without script everything simply stands. */
export function Loop() {
  const root = useRef<HTMLElement>(null);

  useEffect(() => {
    const cards = [...(root.current?.querySelectorAll<HTMLElement>('.loop-card') ?? [])];
    const seen = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          entry.target.setAttribute('data-reveal', 'seen');
          seen.unobserve(entry.target);
        }
      },
      { threshold: 0.35 },
    );
    for (const card of cards) {
      if (card.getBoundingClientRect().top > window.innerHeight) card.setAttribute('data-reveal', 'wait');
      seen.observe(card);
    }
    return () => seen.disconnect();
  }, []);

  return (
    <section ref={root} className="loop wrap" aria-labelledby="loop-title">
      <h2 id="loop-title">One binary, the whole loop</h2>
      <div className="loop-stack">
        {CARDS.map((card, index) => (
          <article key={card.title} className="loop-card" style={{ '--n': index } as CSSProperties}>
            <div className="loop-text">
              <h3>{card.title}</h3>
              <p>{card.text}</p>
              {card.link && (
                <a className="loop-link" href={card.link.href}>
                  {card.link.label}
                </a>
              )}
            </div>
            <pre className="loop-panel" aria-label={`${card.title}: what llx printed`}>
              {card.lines.map((line, row) => (
                <span key={row} className="loop-row" style={{ '--i': row } as CSSProperties}>
                  {line.map(([tone, text], part) => (
                    <span key={part} className={tone ? `t-${tone}` : undefined}>
                      {text}
                    </span>
                  ))}
                </span>
              ))}
            </pre>
          </article>
        ))}
      </div>
    </section>
  );
}
