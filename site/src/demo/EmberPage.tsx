import type { RefObject } from 'react';
import type { PageState } from './usePage';

interface Props {
  state: PageState;
  pageRef: RefObject<HTMLDivElement | null>;
  orderRef: RefObject<HTMLSpanElement | null>;
}

const MENU = [
  ['Espresso', '$3.00'],
  ['Cappuccino', '$4.50'],
  ['Cold Brew', '$4.00'],
] as const;

const on = (base: string, flag: boolean, name = 'is-on') => (flag ? `${base} ${name}` : base);

/** The page the agent drives, a card that lands over the terminal's lower corner. */
export function EmberPage({ state, pageRef, orderRef }: Props) {
  let cursor = on('b-cursor', state.cursorOn);
  if (state.cursorClick) cursor += ' is-click';
  return (
    <div className={on('browser', state.on)} aria-hidden="true">
      <div className="b-url">localhost:8766</div>
      <div ref={pageRef} className="b-page ember">
        <p className="e-title">Ember Coffee</p>
        <p className="e-tag">Small-batch roasts, poured slow.</p>
        <ul className="e-menu">
          {MENU.map(([name, price]) => (
            <li key={name}>
              <span>{name}</span>
              <span>{price}</span>
            </li>
          ))}
        </ul>
        <span ref={orderRef} className={on('e-order', state.pressed, 'is-pressed')}>
          Order
        </span>
        <p className={on('e-thanks', state.thanks)}>Thanks, your order is in!</p>
        <span className={cursor} style={{ transform: state.cursorTransform }} />
      </div>
    </div>
  );
}
