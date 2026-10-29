import { AGENTS_DOCS, CI_DOCS, DOCS } from '../links';
import './Ci.css';

const USES: { who: string; text: string; cmd: string; href: string }[] = [
  {
    who: 'Your CI and scripts',
    text: 'One task, then exit: the answer in the shape you define, JSON events, an exit code to branch on.',
    cmd: 'llx exec --json --schema …',
    href: CI_DOCS,
  },
  {
    who: 'You, at the terminal',
    text: 'A minimal agent with five tools: bash, apply_patch, ask, agents and jobs. Sign in with /login, then give it a task.',
    cmd: 'llx',
    href: DOCS,
  },
  {
    who: 'Your other agents',
    text: 'Commands they run from their shell: a helper on any model, or one choice about stdin in a second.',
    cmd: 'llx exec · llx decide',
    href: AGENTS_DOCS,
  },
];

/** Who calls llx, and with what. */
export function Uses() {
  return (
    <section className="uses wrap" aria-labelledby="uses-title">
      <h2 id="uses-title">One binary, three ways in</h2>
      <ul className="uses-list">
        {USES.map((use) => (
          <li key={use.who}>
            <h3>
              <a href={use.href}>{use.who}</a>
            </h3>
            <p>{use.text}</p>
            <code className="uses-cmd">{use.cmd}</code>
          </li>
        ))}
      </ul>
    </section>
  );
}
