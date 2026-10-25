import { Film } from '../demo/Film';
import { GITHUB, INSTALL } from '../links';
import { InstallCommand } from './InstallCommand';
import './Hero.css';

export function Hero() {
  return (
    <section className="hero wrap">
      <div className="hero-copy">
        <h1>
          Ultrafast, <span className="chip">lightweight</span> coding agent.
        </h1>
        <p className="lede">
          <b>llx</b> writes the code, tests it in a real browser and fixes what it finds. One binary under
          4&nbsp;MB that starts in about 5&nbsp;ms, for your terminal.
        </p>
        <InstallCommand commands={INSTALL} />
        <p className="under">
          macOS · Linux · one binary under 4&nbsp;MB · <a href={`${GITHUB}/blob/main/docs/usage.md`}>Read the docs</a>
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
