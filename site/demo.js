// The demo: an llx session that replays one recorded run at the speed it ran. The banner, the help,
// the trace and the closing lines are written the way llx writes them (apps/cli/src/output/human.rs,
// commands/repl.rs); the page the agent drives opens beside the terminal while it works.
'use strict';

const MODEL = 'anthropic/claude-haiku-5.5';
const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

const HELP = `Type a task and press Enter; each one goes on from the last. End a line with \\ to go on on the next.
  /mode [name]      the modes, or switch to one (also /permissions)
  /plan [task]      plan mode: investigate and answer with a plan; with a task, run it
  /new              start a new conversation and session (also /clear)
  /status           the model, the workspace, the mode, the session and its terminals
  /help             this help
  /exit             quit (also /quit, or Ctrl-D)`;

// The recorded run (llx exec -y, 23 October 2026): every `at` is the millisecond its event
// happened. `out` are the result lines -v shows, `code` the lines of code among them (with the
// diff sign of an edit); `page` drives the page beside the terminal.
const TASK = 'Test the Ember page in the browser: click Order, it should say "Thanks, your order is in!". If it does not, fix index.html and test again.';
const EVENTS = [
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
const END = 13862;
const SUMMARY = [
  ['achieved in 13.9s · 6 turns · $0.0014', 'ok'],
  [`  agent    ${MODEL} · 36,236 in (33,602 cached) · 912 out · $0.0011 · 9.0s`, 'dim'],
  ['  hands    typesafe/jev-1.14 · 4 calls · 6,417 in · 662 out · $0.0003 · 1.7s', 'dim'],
];
const ANSWER = "The bug was in index.html: the script looked up getElementById('order-button'), but the button's id is order, so no click listener was attached. I changed the lookup to getElementById('order'). Before the fix, clicking Order showed no change; now it shows \"Thanks, your order is in!\".";

// A small highlighter for the HTML and JavaScript the run shows: tags, strings, keywords, numbers,
// comments and punctuation. Enough to find the change at a glance; not a parser.
const TOKENS = /(<\/?[a-z!][^\s>]*|>)|('(?:[^'\\]|\\.)*'|"(?:[^"\\]|\\.)*"|`[^`]*`)|(\/\/.*$)|\b(const|let|var|function|return|if|else|for|while|new|class|await|async|true|false|null|undefined)\b|\b(\d+(?:\.\d+)?)\b|([()[\]{}.,;=>])/g;
function highlight(code, into) {
  let last = 0;
  for (const m of code.matchAll(TOKENS)) {
    if (m.index > last) into.append(code.slice(last, m.index));
    const kind = m[1] ? 't' : m[2] ? 's' : m[3] ? 'c' : m[4] ? 'k' : m[5] ? 'n' : 'p';
    const span = document.createElement('span');
    span.className = `tk-${kind}`;
    span.textContent = m[0];
    into.append(span);
    last = m.index + m[0].length;
  }
  if (last < code.length) into.append(code.slice(last));
}

function setUpDemo(root) {
  const q = (selector) => root.querySelector(selector);
  const screen = q('[data-screen]');
  const input = q('[data-input]');
  const ps = q('[data-ps]');
  const browser = q('[data-browser]');
  const b = Object.fromEntries([...root.querySelectorAll('[data-b]')].map((n) => [n.dataset.b, n]));

  let session = 1792700000;
  let tasks = 0;
  let busy = false;
  let token = 0; // anything newer cancels what is playing
  let touched = 0; // when the visitor last typed: the replay waits for them to stop
  let closed = false;

  // One line: plain text, or parts as [text, class]; `hang` is where a wrapped line continues.
  function line(text, kind = '', hang = 0) {
    // A blank line separates; at the top of an empty screen there is nothing to separate.
    if (text === '' && !screen.querySelector('.l')) return null;
    const node = document.createElement('div');
    node.className = `l ${kind}`;
    if (hang) node.style.setProperty('--hang', typeof hang === 'number' ? `${hang}ch` : hang);
    for (const part of Array.isArray(text) ? text : [[text]]) {
      if (typeof part === 'function') {
        part(node);
        continue;
      }
      const [content, className] = part;
      if (!className) {
        node.append(content);
        continue;
      }
      const span = document.createElement('span');
      span.className = className;
      span.textContent = content;
      node.append(span);
    }
    // The prompt line follows the output, as in a real terminal, when it sits in the screen.
    const entry = input.closest('.entry');
    if (entry && entry.parentElement === screen) screen.insertBefore(node, entry);
    else screen.append(node);
    screen.scrollTop = screen.scrollHeight;
    return node;
  }
  // Results sit one step in (`.res`, under the tool's name on a wide screen); CSS sets the step.
  const lines = (text, kind) => text.split('\n').forEach((t) => line(t, kind));
  const time = (ms) => `${(ms / 1000).toFixed(1)}s`.padStart(7, ' ');

  // A new session: an empty screen and the prompt line, nothing else.
  function banner() {
    session += 1;
    tasks = 0;
    clearScreen();
  }
  function clearScreen() {
    screen.querySelectorAll('.l').forEach((node) => node.remove());
  }

  const sleep = (ms, mine) =>
    new Promise((resolve, reject) => {
      setTimeout(() => (mine === token ? resolve() : reject(new Error('cancelled'))), reduced ? Math.min(ms, 1) : ms);
    });

  // ---- the page beside the terminal ----

  function page(what, thanks) {
    if (what === 'open') {
      b.thanks.classList.remove('is-on');
      b.cursor.classList.remove('is-on', 'is-click');
      b.cursor.style.transform = 'translate(70%, 30%)';
      browser.classList.add('is-on');
    } else if (what === 'close') {
      browser.classList.remove('is-on');
    } else if (what === 'click') {
      const frame = b.cursor.parentElement.getBoundingClientRect();
      const box = b.order.getBoundingClientRect();
      b.cursor.classList.add('is-on');
      b.cursor.classList.remove('is-click');
      b.cursor.style.transform = `translate(${box.left - frame.left + box.width * 0.55}px, ${box.top - frame.top + box.height * 0.55}px)`;
      setTimeout(() => {
        b.cursor.classList.add('is-click');
        b.order.classList.add('is-pressed');
        if (thanks) b.thanks.classList.add('is-on');
      }, reduced ? 0 : 300);
      setTimeout(() => b.order.classList.remove('is-pressed'), 460);
    }
  }

  // ---- the recorded run ----

  async function replay(mine) {
    banner();
    await sleep(900, mine);
    input.value = '';
    for (const ch of TASK) {
      await sleep(14 + Math.random() * 22, mine);
      input.value += ch;
      input.scrollLeft = input.scrollWidth;
    }
    await sleep(350, mine);
    input.value = '';
    line('');
    line(`› ${TASK}`, 'cmd');
    busy = true;
    root.classList.add('is-busy');
    tasks += 1;
    let clock = 0;
    for (const e of EVENTS) {
      await sleep(e.at - clock, mine);
      clock = e.at;
      if (e.call) {
        const [tool, args] = e.call;
        line([[time(e.at), 't'], [' ▸ '], [tool, 'tool'], [' '], [args, 'arg']], 'call', 'calc(7ch + 3ch)');
      }
      if (e.step) line([[time(e.at), 't'], ['   '], [e.step]], 'step', 'calc(7ch + 3ch)');
      for (const out of e.out || []) {
        const [text, kind] = Array.isArray(out) ? out : [out];
        line(text, `${kind || 'out'} res`);
      }
      for (const [sign, n, code] of e.code || []) {
        const kind = sign === '-' ? 'del' : sign === '+' ? 'add' : 'src';
        line([[sign, 'sign'], [String(n).padStart(4, ' ') + '  ', 'num'], (node) => highlight(code, node)], `${kind} res`);
      }
      if (e.page) page(e.page, e.thanks);
    }
    await sleep(END - clock, mine);
    line('');
    SUMMARY.forEach(([text, kind]) => line(text, kind));
    line(`  session  ${session}-a3f1`, 'dim');
    line('');
    line(ANSWER, 'answer');
    busy = false;
    root.classList.remove('is-busy');
  }

  // Replay, rest, and replay again, while the visitor is not typing.
  async function loop() {
    for (;;) {
      if (Date.now() - touched > 9000 && !closed) {
        const mine = ++token;
        try {
          await replay(mine);
          await sleep(7000, mine);
        } catch {
          busy = false;
          root.classList.remove('is-busy');
          browser.classList.remove('is-on');
        }
      } else {
        await new Promise((resolve) => setTimeout(resolve, 1000));
      }
    }
  }

  // ---- what a visitor types ----

  function submit(text) {
    const entry = text.trim();
    if (closed) {
      line(`$ ${entry}`, 'cmd');
      if (entry === 'llx' || entry === 'lx') {
        closed = false;
        ps.textContent = '›';
        banner();
      } else if (entry) {
        line(`zsh: command not found: ${entry.split(' ')[0]}`, 'dim');
      }
      return;
    }
    line('');
    line(`› ${entry}`, 'cmd');
    switch (entry.split(/\s+/)[0]) {
      case '':
        return;
      case '/help':
        return lines(HELP, 'dim');
      case '/status':
        return lines(`model      ${MODEL}\ndecision   ~typesafe/jev-latest\nworkspace  ~/ember\nmode       bypass\nsession    ${session}-a3f1 (${tasks} tasks)\nterminals  none`, 'dim');
      case '/new':
      case '/clear':
        return banner();
      case '/exit':
      case '/quit':
        closed = true;
        ps.textContent = '$';
        return line('Session ended. Type llx to start again.', 'dim');
      default:
        if (entry.startsWith('/')) return line(`unknown command ${entry.split(/\s+/)[0]}: /help lists them`, 'bad');
        line('  This page replays a recorded run; install llx to run your own task.', 'dim');
    }
  }

  // The visitor takes over: the replay stops where it is, and resumes once they leave it be.
  function cancel() {
    token += 1;
    busy = false;
    root.classList.remove('is-busy');
    browser.classList.remove('is-on');
  }

  input.addEventListener('keydown', (event) => {
    touched = Date.now();
    if (event.key === 'Enter') {
      event.preventDefault();
      if (busy) cancel();
      const text = input.value;
      input.value = '';
      submit(text);
    } else if (event.key === 'd' && event.ctrlKey) {
      event.preventDefault();
      submit('/exit');
    }
  });
  input.addEventListener('input', (event) => {
    if (!event.isTrusted) return;
    touched = Date.now();
    cancel();
  });
  q('.term').addEventListener('click', () => input.focus({ preventScroll: true }));

  banner();
  if ('IntersectionObserver' in window) {
    const seen = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        seen.disconnect();
        loop();
      }
    }, { threshold: 0.3 });
    seen.observe(root);
  } else {
    loop();
  }
}

// ---- the copy buttons ----

function setUpCopy(button) {
  const root = button.closest('[data-copy-root]');
  const command = root.querySelector('[data-copy-text]');
  const label = button.querySelector('.copy-label');
  button.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(command.textContent.trim());
      label.textContent = 'Copied';
    } catch {
      const range = document.createRange();
      range.selectNodeContents(command);
      const selection = window.getSelection();
      selection.removeAllRanges();
      selection.addRange(range);
      label.textContent = 'Selected';
    }
    button.dataset.done = '';
    setTimeout(() => {
      label.textContent = 'Copy';
      delete button.dataset.done;
    }, 1600);
  });
}

// ---- the meta line under the install command, from benchmarks.json ----

async function setUpMeta(root) {
  let bench;
  try {
    bench = await (await fetch('benchmarks.json', { cache: 'no-cache' })).json();
  } catch {
    return; // the line keeps the values written in the page
  }
  const set = (key, text) => {
    const node = root.querySelector(`[data-meta="${key}"]`);
    if (node && text) node.textContent = text;
  };
  set('version', bench.version && `v${bench.version}`);
  set('binary', bench.binary_mb && `${bench.binary_mb.toFixed(1)} MB`);
  set('start', bench.start_ms && `${Math.round(bench.start_ms)} ms`);
  root.querySelector('[data-bench-sample]').hidden = !bench.mock;
}

document.querySelectorAll('[data-demo]').forEach(setUpDemo);
document.querySelectorAll('[data-copy]').forEach(setUpCopy);
document.querySelectorAll('[data-bench]').forEach(setUpMeta);
