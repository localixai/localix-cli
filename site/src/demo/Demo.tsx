import { Fragment, useMemo, useRef } from 'react';
import type { CSSProperties } from 'react';
import { EmberPage } from './EmberPage';
import { highlight } from './highlight';
import { useSession } from './useSession';
import type { Line } from './useSession';
import './Demo.css';

function TermLine({ line }: { line: Line }) {
  const style = line.hang ? ({ '--hang': line.hang } as CSSProperties) : undefined;
  return (
    <div className={`l ${line.kind}`} style={style}>
      {line.parts.map((part, i) =>
        'code' in part ? (
          <Fragment key={i}>{highlight(part.code)}</Fragment>
        ) : part.cls ? (
          <span key={i} className={part.cls}>
            {part.text}
          </span>
        ) : (
          part.text
        ),
      )}
    </div>
  );
}

/** The recorded run, replayed in a terminal with the page it tests landing beside it. */
export function Demo() {
  const root = useRef<HTMLDivElement>(null);
  const screen = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const page = useRef<HTMLDivElement>(null);
  const order = useRef<HTMLSpanElement>(null);
  const refs = useMemo(() => ({ root, screen, input, page, order }), []);
  const session = useSession(refs);

  return (
    <div
      ref={root}
      className={`hero-demo demo${session.busy ? ' is-busy' : ''}`}
      aria-label="llx at work, replayed from a recorded run"
    >
      <div className="rig">
        <div className="term" onClick={session.focus}>
          <div ref={screen} className="screen" role="log" aria-live="off">
            {session.lines.map((line) => (
              <TermLine key={line.id} line={line} />
            ))}
            <label className="entry">
              <span className="ps" aria-hidden="true">
                {session.closed ? '$' : '›'}
              </span>
              <input
                ref={input}
                type="text"
                value={session.input}
                onChange={session.onChange}
                onKeyDown={session.onKeyDown}
                autoComplete="off"
                autoCapitalize="off"
                spellCheck={false}
                enterKeyHint="send"
                aria-label="Type a task or a slash command"
              />
            </label>
          </div>
        </div>
        <EmberPage state={session.page} pageRef={page} orderRef={order} />
      </div>
    </div>
  );
}
