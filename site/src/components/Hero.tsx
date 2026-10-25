import { Demos } from '../demo/Demos';
import { INSTALL } from '../links';
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
        <InstallCommand command={INSTALL} />
        <p className="under">
          macOS · Linux · or <code>npm install -g @rsaryev/llx</code>
        </p>
      </div>
      <Demos />
    </section>
  );
}
