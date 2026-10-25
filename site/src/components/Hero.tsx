import { Film } from '../demo/Film';
import { GITHUB, INSTALL } from '../links';
import { InstallCommand } from './InstallCommand';
import './Hero.css';

export function Hero() {
  return (
    <section className="hero wrap">
      <div className="hero-copy">
        <h1>
          The <span className="chip">ultrafast</span> browser and coding agent.
        </h1>
        <p className="lede">
          <b>llx</b> is a native browser, terminals and a coding agent in one binary under 4&nbsp;MB. It
          writes the code, tests it in the browser and fixes what it finds; <code>llx mcp</code> gives the same
          window to Claude Code, Codex or any agent.
        </p>
        <InstallCommand tabs={INSTALL} />
        <p className="under">
          macOS · Linux · under 4&nbsp;MB · <a href={`${GITHUB}#use-it-from-another-agent`}>MCP setup</a> ·{' '}
          <a href={`${GITHUB}/blob/main/docs/usage.md`}>Read the docs</a>
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
