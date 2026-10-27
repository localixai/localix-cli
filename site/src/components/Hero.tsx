import { DOCS, INSTALL } from '../links';
import { InstallCommand } from './InstallCommand';
import { Terminal } from './Shell';
import './Hero.css';

/** Two calls an agent makes from its shell, with what a real run printed. */
const SESSION = `$ llx exec "Which file in src/ parses the command line? Its path only."
src/cli.rs
$ git show --stat | llx decide --choices patch,minor,major "Which version bump?"
minor`;

export function Hero() {
  return (
    <section className="hero wrap">
      <h1>More hands for the agent you already use.</h1>
      <p className="lede">
        llx is one small binary that Claude Code, Codex, a script or CI calls from its shell: helpers on any model and
        a choice made in a second.
      </p>
      <div className="hero-start">
        <InstallCommand tabs={INSTALL} />
        <a className="hero-docs" href={DOCS}>
          Read the docs
        </a>
      </div>
      <figure className="hero-session">
        <Terminal wrap session={SESSION} label="Two llx calls from an agent's shell, and what they printed" />
        <figcaption>Outputs shortened.</figcaption>
      </figure>
    </section>
  );
}
