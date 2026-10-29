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
          <span className="hero-chip">Autonomous</span> agent runtime for CI and scripts
        </h1>
        <p className="lede">
          One task, a structured end every time, and an exit code to branch on. One small binary on any model, with a minimal
          agent for your terminal, and commands other agents can call.
        </p>
        <InstallCommand tabs={INSTALL} />
        <p className="hero-under">
          macOS · Linux · MIT · <a href={DOCS}>Read the docs</a>
        </p>
      </div>
      <figure className="hero-demo">
        <Film src="/demo-llx.cast" label="An llx session picks its model with /model and hands a fix to two agents; then llx exec checks the change as CI would" />
      </figure>
    </section>
  );
}
