import { Mock } from './Mock';
import './Window.css';

const USE: [string, string][] = [
  ['Open', 'The agent opens pages and terminals as tabs, each project’s together.'],
  ['Say the goal', '“Book the cheapest flight on Friday”: a small, fast model does the clicking.'],
  ['Watch', 'Every tab stays live while you go on typing elsewhere.'],
  ['Point', 'Click what is wrong, write a note, Send. The agent that opened the page fixes it.'],
];

const GETS = [
  'the element’s selector and path',
  'its source file and component, from your dev build',
  'its box, role, text and key styles',
  'a picture of it, and your note',
];

/** The window, drawn: tabs of pages and terminals, the agent at work, the person's comments. */
export function Window() {
  return (
    <section className="window wrap" aria-labelledby="window-title">
      <h2 id="window-title">One window you can watch.</h2>
      <p className="window-lede">
        Pages in the system’s own WebView and terminals in a real PTY, as tabs of one native window, each
        project’s together. It never takes your keyboard.
      </p>

      <Mock />

      <div className="window-lists">
        <div>
          <h3>How it works</h3>
          <ol>
            {USE.map(([name, text]) => (
              <li key={name}>
                <b>{name}.</b> {text}
              </li>
            ))}
          </ol>
        </div>
        <div>
          <h3>What the agent gets from your comment</h3>
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
