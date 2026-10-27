import { DOCS, INSTALL } from '../links';
import { InstallCommand } from './InstallCommand';
import { Terminal } from './Shell';
import './Hero.css';

/** Four calls an agent makes from its shell, with what a real run printed (shortened: the served
 *  page's address, the helper's trace on stderr). */
const SESSION = `$ llx window '{"action":"open","kind":"browser","url":"demo/shop.html"}'
w1 (browser): Opened demo/shop.html.
$ llx window '{"action":"interact","tab":"w1","goal":"add the cheapest item to the cart"}'
Helper: done (100% sure)
1. CLICK button "Add Mint to cart" [e21] · 99%
2. DONE · 100%
$ llx exec "Which page in demo/ has a date-range picker? Its path only."
demo/booking.html
$ git show --stat | llx decide --choices patch,minor,major "Which version bump?"
minor`;

export function Hero() {
  return (
    <section className="hero wrap">
      <h1>More hands for the agent you already use.</h1>
      <p className="lede">
        llx is one small binary that Claude Code, Codex, a script or CI calls from its shell: browser pages and
        terminals you watch it work in, helpers on any model, and a choice made in a second.
      </p>
      <div className="hero-start">
        <InstallCommand tabs={INSTALL} />
        <a className="hero-docs" href={DOCS}>
          Read the docs
        </a>
      </div>
      <figure className="hero-session">
        <Terminal wrap session={SESSION} label="Four llx calls from an agent's shell, and what they printed" />
        <figcaption>From a real run on the repository&apos;s demo shop, outputs shortened.</figcaption>
      </figure>
    </section>
  );
}
