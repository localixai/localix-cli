// The usage statistics card: one sentence and two equal buttons, over the page's lower corner. It
// is plain DOM and a few listeners, not React, so a docs page (no React in the browser) shows the
// same card as the landing. It is a named region, not a dialog: it takes no focus at a page's
// load, blocks nothing, and is fixed, so its arrival moves nothing on the page.
import { ANALYTICS } from '../links';
import { answer, measurementId, privacySignal, resumeAnalytics, storedAnswer, type Answer } from '.';

type Copy = { notice: string; privacy?: string };

const CARD = 'consent';
/** The attribute of anything that opens the card again: the footer's "Cookie settings". */
export const OPEN = 'data-consent-open';

/** At a page's load: measure if this device said yes; ask if it has not answered and sent no
 *  privacy signal; open the card again from anything marked `data-consent-open`. */
export function startConsent(): void {
  const copy = ANALYTICS;
  if (!copy || !measurementId()) return;
  resumeAnalytics();
  if (!storedAnswer() && !privacySignal()) show(copy, false);
  document.addEventListener('click', (event) => {
    if (!(event.target instanceof Element) || !event.target.closest(`[${OPEN}]`)) return;
    event.preventDefault();
    show(copy, true);
  });
}

function show(copy: Copy, asked: boolean): void {
  const card = document.getElementById(CARD) ?? document.body.appendChild(build(copy));
  // Asked for from the footer, the card is where the visitor's attention is: focus its first choice.
  if (asked) card.querySelector('button')?.focus();
}

function build(copy: Copy): HTMLElement {
  const card = Object.assign(document.createElement('section'), { id: CARD, className: 'consent' });
  card.setAttribute('aria-label', 'Usage statistics');
  const text = card.appendChild(document.createElement('p'));
  text.textContent = copy.notice;
  if (copy.privacy) {
    text.append(' ', Object.assign(document.createElement('a'), { href: copy.privacy, textContent: 'Privacy' }));
  }
  const actions = card.appendChild(Object.assign(document.createElement('div'), { className: 'consent-actions' }));
  const button = (value: Answer, label: string) => {
    const choice = Object.assign(document.createElement('button'), { type: 'button', textContent: label });
    choice.addEventListener('click', () => {
      answer(value);
      card.remove();
    });
    actions.append(choice);
  };
  button('denied', 'No thanks');
  button('granted', 'Allow');
  return card;
}
