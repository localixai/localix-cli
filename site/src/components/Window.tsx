import { Mock } from './Mock';
import './Window.css';

const USE: [string, string][] = [
  ['Open', 'The agent opens pages and terminals as tabs, each project’s together.'],
  ['Watch', 'Every tab stays live: each click, each key, each command as it runs.'],
  ['Comment', 'Press the comment icon, click what is wrong, write a note. Repeat.'],
  ['Send', 'Send hands the batch to the agent that opened the page, and it fixes them.'],
];

const GETS = [
  'the element’s CSS selector and path',
  'its source file and component, from your dev build',
  'its box, role, text and computed styles',
  'a picture of it, and your note',
];

/** The window, drawn: tabs of pages and terminals, the agent at work, the person's comments. */
export function Window() {
  return (
    <section className="window wrap" aria-labelledby="window-title">
      <h2 id="window-title">Everything in one window.</h2>
      <p className="window-lede">
        Browser pages and terminals are tabs of one native window, grouped by project. Give the agent a goal and
        it is done in seconds: find and book a flight, fill a form, check a page. In the next tab it runs your
        tests, servers and REPLs, typing and reading as you would, without a second app. llx works there, and so
        do Claude Code and Codex through <code>llx mcp</code>. You watch every tab, and point at what to change.
      </p>

      <Mock />

      <div className="window-lists">
        <div>
          <h3>How you use it</h3>
          <ol>
            {USE.map(([name, text]) => (
              <li key={name}>
                <b>{name}.</b> {text}
              </li>
            ))}
          </ol>
        </div>
        <div>
          <h3>What the agent gets</h3>
          <ul>
            {GETS.map((text) => (
              <li key={text}>{text}</li>
            ))}
          </ul>
          <p className="window-vs">
            Not &ldquo;the blue button at the bottom&rdquo;, but <code>form#checkout &gt; button.submit</code> in{' '}
            <code>src/Checkout.tsx:42</code>.
          </p>
        </div>
      </div>
    </section>
  );
}
