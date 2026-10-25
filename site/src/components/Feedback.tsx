import './Feedback.css';

const STEPS: [string, string][] = [
  ['Comment', 'Turn it on in the window’s strip, then click any element or drag over a region.'],
  ['Say what is wrong', 'A note on each. llx keeps the element’s selector, box, styles, source file and a picture.'],
  ['Send', 'The batch goes back to the agent that opened the page: llx itself, or Claude Code through llx mcp.'],
];

/** Visual feedback: the person points at what is wrong in the window, the agent gets it all. */
export function Feedback() {
  return (
    <section className="agents feedback wrap" aria-labelledby="feedback-title">
      <h2 id="feedback-title">Point at it. The agent gets the details.</h2>
      <p>
        Click what looks wrong in the page the agent opened, say why, and send. Each comment carries what an agent needs to
        find it in the code, so &ldquo;this button&rdquo; is enough.
      </p>
      <div className="fb-shot" aria-hidden="true">
        <div className="fb-strip">
          <span className="fb-tab">localhost:5173</span>
          <span className="fb-spacer" />
          <span className="fb-send">Send 2</span>
          <span className="fb-toggle">Comment</span>
        </div>
        <div className="fb-page">
          <div className="fb-h">Checkout</div>
          <div className="fb-row" />
          <div className="fb-row short" />
          <span className="fb-btn">Pay now</span>
          <div className="fb-card">
            <b>2</b> The button is too small on a phone, make it full width.
          </div>
        </div>
      </div>
      <ol className="fb-steps">
        {STEPS.map(([name, text], i) => (
          <li key={name}>
            <span>{i + 1}</span>
            <div>
              <b>{name}</b>
              <p>{text}</p>
            </div>
          </li>
        ))}
      </ol>
    </section>
  );
}
