import { type CSSProperties, useEffect, useRef } from 'react';
import { GITHUB } from '../links';
import './Loop.css';

/** A line of a panel: its text in parts, each with a tone (the terminal's colours). */
type Tone = 'dim' | 'hi' | 'ok' | 'bad' | 'acc' | 'sel' | '';
type Line = [Tone, string][];

type Card = { title: string; text: string; lines: Line[]; link?: { href: string; label: string } };

const EXAMPLES = `${GITHUB}/blob/main/examples`;

/** What each step printed in the recorded runs (public/demo-llx.cast and its takes), shortened; the
 *  benchmark card is benchmarks/arena's two runs (medians); the review card is examples/github/review.yml, the API card examples/breaking-changes' recorded
 *  run (demo/output.txt); the rules card is put together from llx's own wording of a refusal
 *  (packages/tools/src/lib.rs). */
const CARDS: Card[] = [
  {
    title: 'Less context, same model',
    text: 'One task: make a Bun, React and Vite app match its spec, checked by 9 tests. llx and Codex in turns on the same model, low reasoning; medians of 5 runs each.',
    link: { href: `${GITHUB}/tree/main/benchmarks/arena`, label: 'the runs' },
    lines: [
      [['dim', '                    Luna 6           Terra 6.1']],
      [['dim', '              llx    Codex        llx    Codex']],
      [['', 'context '], ['ok', '      47k'], ['', '     211k'], ['ok', '        33k'], ['', '      81k']],
      [['', 'time    '], ['ok', '    28.6s'], ['', '    43.2s'], ['ok', '      25.2s'], ['', '    31.4s']],
      [['', 'cost    '], ['ok', '  $0.0031'], ['', '  $0.0052'], ['ok', '     $0.019'], ['', '   $0.021']],
      [['', 'passed  '], ['', '      4/5'], ['ok', '      5/5'], ['', '        5/5'], ['', '      5/5']],
    ],
  },
  {
    title: 'Any model, any provider',
    text: 'Pick a provider and a model with /model: a ChatGPT sign-in, OpenRouter, Anthropic, xAI, DeepSeek. The reasoning level with ← →, saved for every run after.',
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
    title: 'Agents in parallel',
    text: 'One task, split: each agent is a session of its own, running at once, answering back to the one that started it.',
    lines: [
      [['acc', '• '], ['hi', 'Spawned'], ['', ' median · why the median tests fail']],
      [['acc', '↳ '], ['hi', 'Spawned'], ['', ' mean · mean() on an empty list']],
      [['ok', '● '], ['', 'Agent "median" finished · 6s']],
      [['ok', '● '], ['', 'Agent "mean" finished · 6s']],
      [['ok', '• '], ['hi', 'Patched'], ['', ' stats.py '], ['dim', '(+1 -1 lines)']],
      [['', '• Fixed '], ['acc', 'stats.py:15'], ['', ': averages the two middle values.']],
    ],
  },
  {
    title: 'Checks a change in CI',
    text: 'Reads the diff, runs the tests, changes nothing, and answers with data your pipeline reads: JSON that matches your schema.',
    lines: [
      [['dim', '$ '], ['', 'git diff | llx exec -y --deny apply_patch \\']],
      [['', '    --schema @ship.json "Safe to ship? Which bump?"']],
      [['ok', '• '], ['hi', 'Ran'], ['', ' python3 -m unittest -v']],
      [['dim', '  └ Ran 4 tests in 0.000s · OK']],
      [['', '{"bump":"patch","ship":true,']],
      [['', ' "why":"Fixes even-length median to match its docs"}']],
    ],
  },
  {
    title: 'Reviews every pull request',
    text: 'A GitHub workflow: the agent reads the change, runs what it needs to confirm a problem, and a second job, with no model key, posts inline comments and one summary.',
    link: { href: `${EXAMPLES}/github/review.yml`, label: 'review.yml and six more workflows' },
    lines: [
      [['acc', '- uses: '], ['', 'rsaryev/llx@v0']],
      [['acc', '  with:']],
      [['', '    allow: bash']],
      [['', '    deny: apply_patch']],
      [['', '    prompt: |']],
      [['', '      Review pull request #$PR of $GH_REPO.']],
      [['', '      Find real problems: bugs, security flaws, lost data.']],
      [['', '      Before you report a problem, confirm it.']],
      [['', '    schema: '], ['dim', '{ findings: [...], summary }']],
    ],
  },
  {
    title: 'Catches breaking API changes',
    text: 'A GitLab job on every merge request: llx reads the diff, the spec and the code behind it, read-only, and says what breaks the API’s clients, even what no spec diff sees, like a unit or an order that changed.',
    link: { href: `${EXAMPLES}/breaking-changes/README.md`, label: 'breaking-changes, with its GitLab job' },
    lines: [
      [['dim', '$ '], ['', 'llx exec --deny apply_patch --schema @schema.json "$prompt"']],
      [['bad', 'BREAKS'], ['', '  structural  POST /orders: 201 became 200']],
      [['bad', 'BREAKS'], ['', '  semantic    Order.amount: major units now cents']],
      [['bad', 'BREAKS'], ['', "  semantic    'skip' is a page index, not an offset"]],
      [['dim', '…  12 breaking in all']],
      [['ok', 'safe  '], ['', '  structural  new optional query parameter currency']],
      [['dim', '(exit 1: something breaks)']],
    ],
  },
  {
    title: 'Decides and writes in a second',
    text: 'llx decide picks one of the options you give, alone on stdout. A short exec writes the commit message, the label, the note.',
    lines: [
      [['dim', '$ '], ['', 'git diff | llx decide --choices patch,minor,major \\']],
      [['', '    "Which version bump?"']],
      [['ok', 'patch']],
      [['dim', '$ '], ['', "echo 'App crashes when the cart is empty' |"]],
      [['', '    llx decide --choices bug,feature,question "Label it"']],
      [['ok', 'bug']],
      [['dim', '$ '], ['', 'git diff | llx exec -y --deny apply_patch "A one-line commit message"']],
      [['', 'Fix median calculation for even-length lists']],
    ],
  },
  {
    title: 'Runs under your rules',
    text: 'A mode is a set of rules: what runs without asking, what is refused in every mode. They come from your command line and settings, never from the repository.',
    lines: [
      [['hi', 'default'], ['dim', '  everything is asked']],
      [['hi', 'plan   '], ['dim', '  file changes refused']],
      [['hi', 'bypass '], ['dim', '  everything runs (-y)']],
      [['dim', '$ '], ['', "llx exec --allow 'bash(cargo test *)' \\"]],
      [['', "    --deny 'bash(git push *)' \"Fix the failing test\""]],
      [['bad', '• '], ['hi', 'Ran'], ['', ' git push']],
      [['bad', '  └ not allowed: refused by the rule `bash(git push *)`']],
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
                  {card.link.label} →
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
