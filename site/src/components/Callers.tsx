import { CI_DOCS, RECIPES_DOCS } from '../links';
import { Recipes } from './Recipes';
import './Sections.css';

const CALLERS: [string, string, string][] = [
  ['You', 'llx', 'A minimal agent at your terminal, with seven tools: read, edit, bash, window, ask, agents, jobs.'],
  [
    'Your scripts and CI',
    'rsaryev/llx · llx decide · llx exec',
    'A GitHub Action, a choice in under a second, or one task with JSON in your shape. Exit codes a pipeline branches on.',
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
  ['130', 'interrupted'],
];

/** llx as a step of a pipeline: the action, a decision with a schema, and the exit codes. */
export function Pipeline() {
  return (
    <section id="ci" className="pipeline wrap" aria-labelledby="ci-title">
      <h2 id="ci-title">A step in any pipeline.</h2>
      <p className="section-lede">
        In CI llx is the agent you use at your terminal, working through the shell: <code>gh</code>,{' '}
        <code>git</code>, <code>jq</code> and your project&apos;s own tools. No MCP server, no GitHub tool, no built-in
        prompt: your workflow owns the prompt and the posting. The job that runs the model holds a read-only token and
        answers in the JSON shape you give it; a second job posts it, so the write token is never near the model.{' '}
        <code>llx decide</code> is the quick version: one question about stdin, one option back, in under a second.
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
        <a href={CI_DOCS}>The action, rules, security and GitLab →</a> ·{' '}
        <a href={RECIPES_DOCS}>Shell recipes with gh and glab →</a>
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
        Reading never asks; every command, file change and page action does, by mode: default, plan, bypass or one
        of your own. A ChatGPT sign-in, OpenRouter, or a key for any provider of the models.dev catalog.
      </p>
    </section>
  );
}
