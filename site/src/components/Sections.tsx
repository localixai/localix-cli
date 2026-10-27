import { Film } from '../demo/Film';
import { AGENTS_DOCS, CI_DOCS, RECIPES_DOCS } from '../links';
import { Recipes } from './Recipes';
import './Sections.css';

const BLOCKS: [string, string, string][] = [
  [
    'llx exec',
    'A task done by a helper, on any model.',
    'It reads, edits and runs what you allow, and prints the answer alone, in your JSON shape if you give one. Run several in the background.',
  ],
  [
    'llx decide',
    'A yes or no about stdin, in a second.',
    'Or one of the choices you name. Exit codes a script or a job branches on.',
  ],
];

/** The commands another agent composes. */
export function Blocks() {
  return (
    <section className="section blocks wrap" aria-labelledby="blocks-title">
      <div className="sec-head">
        <h2 id="blocks-title">Two commands. Your agent composes them.</h2>
        <p>
          Each is one call from a shell, so anything that runs a shell can use them: Claude Code, Codex, a script, a
          job.
        </p>
      </div>
      <dl className="blocks-list">
        {BLOCKS.map(([command, what, more]) => (
          <div key={command}>
            <dt>
              <code>{command}</code>
            </dt>
            <dd>
              <b>{what}</b> {more}
            </dd>
          </div>
        ))}
      </dl>
      <p className="section-more">
        <a className="more" href={AGENTS_DOCS}>
          llx in other agents
        </a>
      </p>
    </section>
  );
}

const CODES: [string, string][] = [
  ['0', 'achieved'],
  ['1', 'not achieved'],
  ['2', 'out of turns or time'],
  ['3', 'not ready'],
  ['130', 'interrupted'],
];

/** llx as a step of a pipeline: the action's workflows, and the exit codes. */
export function Pipeline() {
  return (
    <section id="ci" className="section pipeline wrap" aria-labelledby="ci-title">
      <div className="sec-head">
        <h2 id="ci-title">A step in any pipeline.</h2>
        <p>
          In CI llx works through the shell, with <code>gh</code>, <code>git</code> and your project&apos;s tools. Your
          workflow owns the prompt and the posting: the job with the model holds a read-only token, and a second job
          posts what it answered.
        </p>
      </div>
      <Recipes />
      <dl className="pipeline-codes" aria-label="Exit codes">
        {CODES.map(([code, meaning]) => (
          <div key={code}>
            <dt>{code}</dt>
            <dd>{meaning}</dd>
          </div>
        ))}
      </dl>
      <p className="section-more">
        <a className="more" href={CI_DOCS}>
          The action, rules and security
        </a>
        <a className="more" href={RECIPES_DOCS}>
          Recipes with gh and glab
        </a>
      </p>
    </section>
  );
}

const FACTS: [string, string][] = [
  ['under 4 MB', 'one native binary'],
  ['~5 ms', 'to start'],
  ['$0.002', 'a median task in our evals'],
  ['~200', 'model providers'],
];

/** llx's own agent at the terminal, recorded, and what it costs to carry. */
export function Own() {
  return (
    <section className="section own wrap" aria-labelledby="own-title">
      <div className="sec-head">
        <h2 id="own-title">And an agent of its own.</h2>
        <p>
          Type <code>llx</code> for a minimal agent at your terminal: six tools (read, edit, bash, ask, agents,
          jobs), and modes that say what runs without asking. A ChatGPT sign-in, OpenRouter, or a key for
          any provider of the models.dev catalog.
        </p>
      </div>
      <div className="own-body">
        <dl className="own-facts">
          {FACTS.map(([value, what]) => (
            <div key={value}>
              <dt>{value}</dt>
              <dd>{what}</dd>
            </div>
          ))}
        </dl>
        <figure className="own-film">
          <Film src="/demo-llx.cast" label="A real llx session, recorded with asciinema" />
          <figcaption>
            <code>/login</code>, the modes and a task, recorded live
          </figcaption>
        </figure>
      </div>
    </section>
  );
}
