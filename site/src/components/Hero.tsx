import { Film } from '../demo/Film';
import { DOCS, INSTALL } from '../links';
import { InstallCommand } from './InstallCommand';
import './Hero.css';

export function Hero() {
  return (
    <section className="hero wrap">
      <div className="hero-copy">
        <h1>
          <span className="nowrap"><span className="chip">Ultrafast</span>,</span> lightweight agent runtime.
        </h1>
        <p className="lede">
          A native window of real browser pages and terminals that any agent can work in. A decision step for
          your scripts and CI. A minimal agent for your terminal. One binary under 4&nbsp;MB, no plugins.
        </p>
        <InstallCommand tabs={INSTALL} />
        <p className="under">
          macOS · Linux · starts in 5&nbsp;ms · <a href="#ci">In CI</a> ·{' '}
          <a href={DOCS}>Read the docs</a>
        </p>
      </div>
      <div id="demo" className="hero-demo">
        <Film src="/demo-llx.cast" label="A real llx session, recorded with asciinema" />
        <p className="demo-caption">
          <span>
            <code>/login</code>, the modes and a task, in a real session
          </span>
          <span>Recorded live with asciinema</span>
        </p>
      </div>
    </section>
  );
}
