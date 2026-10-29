import { type CSSProperties, useEffect, useRef } from 'react';
import { DOCS, GITHUB } from '../links';
import './Loop.css';

/** A line of a panel: its text in parts, each with a tone (the terminal's colours). */
type Tone = 'dim' | 'hi' | 'ok' | 'bad' | 'acc' | 'sel' | '';
type Line = [Tone, string][];

type Card = { title: string; text: string; lines: Line[]; link?: { href: string; label: string } };

/** The benchmark card is benchmarks/task-tracker's runs (medians); the rules card uses llx's own
 *  wording of a refusal (packages/tools/src/lib.rs). */
const CARDS: Card[] = [
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
      <h2 id="loop-title">Bounded, measured, under rules</h2>
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
