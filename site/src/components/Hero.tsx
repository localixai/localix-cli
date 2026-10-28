import { DOCS, INSTALL } from '../links';
import { Film } from '../demo/Film';
import { InstallCommand } from './InstallCommand';
import './Hero.css';

/** The landing, one screen: what llx is on the left, a real run of it on the right. */
export function Hero() {
  return (
    <section className="hero wrap">
      <div className="hero-copy">
        <h1>
          <span className="hero-chip">Ultrafast</span>, lightweight agent runtime
        </h1>
        <p className="lede">
          One small binary that Claude Code, Codex, a script or CI calls from its shell: a task done on any model, a
          choice made in a second.
        </p>
        <InstallCommand tabs={INSTALL} />
        <p className="hero-under">
          macOS · Linux · Windows · MIT · <a href={DOCS}>Read the docs</a>
        </p>
      </div>
      <figure className="hero-demo">
        <Film src="/demo-llx.cast" label="An llx session picks its model with /model and hands a fix to two agents; then llx exec checks the change as CI would" />
      </figure>
    </section>
  );
}
