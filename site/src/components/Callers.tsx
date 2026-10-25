import { Recipes } from './Recipes';
import './Sections.css';

const CALLERS: [string, string, string][] = [
  ['You', 'llx', 'A minimal agent at your terminal, with seven tools: read, edit, bash, window, ask, agents, jobs.'],
  [
    'Your scripts and CI',
    'llx decide · llx exec',
    'A choice in under a second, or one task read-only with JSON in your shape. Exit codes a pipeline branches on.',
  ],
  [
    'Your other agents',
    'llx mcp',
    'The same window, served over MCP to Claude Code, Codex, Cursor or any client. Their model thinks; llx acts.',
  ],
];

/** Who calls llx, and what each gets from the same binary. */
export function Callers() {
  return (
    <section className="callers wrap" aria-labelledby="callers-title">
      <h2 id="callers-title">Whoever calls it, it is the same binary.</h2>
      <div className="callers-grid">
        {CALLERS.map(([who, command, what]) => (
          <article key={who}>
            <h3>{who}</h3>
            <code>{command}</code>
            <p>{what}</p>
          </article>
        ))}
      </div>
    </section>
  );
}

const CODES: [string, string][] = [
  ['0', 'achieved'],
  ['1', 'not achieved, or not the --expect option'],
  ['2', 'out of turns or time'],
  ['3', 'not ready: no key, a bad argument'],
];

/** llx as a step of a pipeline: a read-only decision with a schema, and the exit codes. */
export function Pipeline() {
  return (
    <section id="ci" className="pipeline wrap" aria-labelledby="ci-title">
      <h2 id="ci-title">A step in any pipeline.</h2>
      <p className="section-lede">
        <code>llx decide</code> asks a decision model one question about stdin and prints the option it picked, in
        under a second for a fraction of a cent. When the step must read the code, <code>llx exec</code> runs the
        agent read-only and answers in the JSON shape you give it. No review bot to install: <code>gh</code>,{' '}
        <code>glab</code> and <code>git</code> read the input and post the answer.
      </p>
      <Recipes />
      <dl className="pipeline-codes">
        {CODES.map(([code, meaning]) => (
          <div key={code}>
            <dt>{code}</dt>
            <dd>{meaning}</dd>
          </div>
        ))}
      </dl>
      <p className="section-more">
        <a href="https://github.com/rsaryev/llx/blob/main/docs/recipes.md">Every recipe, with GitHub Actions and GitLab CI →</a>
      </p>
    </section>
  );
}

const FACTS: [string, string][] = [
  ['under 4 MB', 'one native binary, Rust'],
  ['~5 ms', 'to start'],
  ['8.5 s · $0.002', 'median task in our evals'],
  ['~200', 'model providers'],
];

/** What it costs to carry: size, start, a task, providers. */
export function Small() {
  return (
    <section className="small wrap" aria-labelledby="small-title">
      <h2 id="small-title">Seven tools. No plugins. Small on purpose.</h2>
      <dl className="small-facts">
        {FACTS.map(([value, what]) => (
          <div key={value}>
            <dt>{value}</dt>
            <dd>{what}</dd>
          </div>
        ))}
      </dl>
      <p className="section-lede">
        Reading never asks; every command, file change and page action does, by mode: default, plan, bypass or
        autonomous. A ChatGPT sign-in, OpenRouter, or a key for any provider of the models.dev catalog.
      </p>
    </section>
  );
}
