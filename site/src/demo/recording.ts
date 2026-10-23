// The recorded run the demo replays (llx exec -y, 23 October 2026). The banner, the help, the trace
// and the closing lines are written the way llx writes them (apps/cli/src/output/human.rs,
// commands/repl.rs).

export const MODEL = 'anthropic/claude-haiku-5.5';

export const HELP = `Type a task and press Enter; each one goes on from the last. End a line with \\ to go on on the next.
  /mode [name]      the modes, or switch to one (also /permissions)
  /plan [task]      plan mode: investigate and answer with a plan; with a task, run it
  /new              start a new conversation and session (also /clear)
  /status           the model, the workspace, the mode, the session and its terminals
  /help             this help
  /exit             quit (also /quit, or Ctrl-D)`;

export type PageAction = 'open' | 'click' | 'close';
/** A result line: plain, or with the line's own class (`ok`). */
export type Out = string | readonly [text: string, kind: string];
/** A line of code among the results, with the diff sign of an edit (' ' for unchanged). */
export type CodeLine = readonly [sign: ' ' | '-' | '+', line: number, code: string];

export interface RunEvent {
  /** The millisecond the event happened, from the start of the run. */
  at: number;
  call?: readonly [tool: string, args: string];
  step?: string;
  /** The result lines -v shows. */
  out?: readonly Out[];
  code?: readonly CodeLine[];
  /** What the page beside the terminal does. */
  page?: PageAction;
  thanks?: boolean;
}

export const TASK =
  'Test the Ember page in the browser: click Order, it should say "Thanks, your order is in!". If it does not, fix index.html and test again.';

export const EVENTS: readonly RunEvent[] = [
  { at: 1507, call: ['read', 'index.html'], out: ['index.html (lines 1-34 of 34)'],
    code: [[' ', 1, '<!doctype html>'], [' ', 2, '<html lang="en">'], [' ', 3, '<head>']] },
  { at: 3343, call: ['bash', '(python3 -m http.server 8766 > /tmp/ember.log 2>&1 &); sleep 2; curl -s -o /dev/null -w "%{http_code}" localhost:8766'] },
  { at: 5413, out: ['exit 0 in 2.1s', '### stdout', ['200', 'ok']] },
  { at: 6699, call: ['browser', 'url: http://localhost:8766 · goal: Click the Order button; done when the page has been clicked once.'], page: 'open' },
  { at: 7680, step: 'CLICK button "Order" [e1] · 100% · no visible change', page: 'click' },
  { at: 8064, step: 'DONE · 89%' },
  { at: 8600, page: 'close' },
  { at: 9657, call: ['edit', 'index.html'], out: ['Edited index.html: replaced 1 occurrence (+1 -1 lines).'],
    code: [['-', 29, "document.getElementById('order-button').addEventListener('click', () => {"], ['+', 29, "document.getElementById('order').addEventListener('click', () => {"]] },
  { at: 10854, call: ['browser', 'url: http://localhost:8766 · goal: Click the Order button; done when the text "Thanks, your order is in!" is visible on the page.'], page: 'open' },
  { at: 11356, step: 'CLICK button "Order" [e1] · 100%', page: 'click', thanks: true },
  { at: 11731, step: 'DONE · 100%' },
  { at: 13300, page: 'close' },
];

export const END = 13862;

export const SUMMARY: readonly (readonly [text: string, kind: string])[] = [
  ['achieved in 13.9s · 6 turns · $0.0014', 'ok'],
  [`  agent    ${MODEL} · 36,236 in (33,602 cached) · 912 out · $0.0011 · 9.0s`, 'dim'],
  ['  hands    typesafe/jev-1.14 · 4 calls · 6,417 in · 662 out · $0.0003 · 1.7s', 'dim'],
];

export const ANSWER =
  "The bug was in index.html: the script looked up getElementById('order-button'), but the button's id is order, so no click listener was attached. I changed the lookup to getElementById('order'). Before the fix, clicking Order showed no change; now it shows \"Thanks, your order is in!\".";
