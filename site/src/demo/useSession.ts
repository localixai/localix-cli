import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { ChangeEvent, KeyboardEvent, RefObject } from 'react';
import { ANSWER, END, EVENTS, HELP, MODEL, SUMMARY, TASK } from './recording';
import { usePage } from './usePage';

// The demo's engine: an llx session that replays the recorded run at the speed it ran, rests, and
// replays again, while the visitor is not typing. A visitor who types takes over: the replay stops
// where it is, a few commands answer as llx would, and the replay resumes once they leave it be.

/** A part of a line: text with an optional class, or code to highlight. */
export type Part = { text: string; cls?: string } | { code: string };

export interface Line {
  id: number;
  kind: string;
  /** Where a wrapped line continues, as a CSS length. */
  hang?: string;
  parts: Part[];
}

export interface DemoRefs {
  root: RefObject<HTMLDivElement | null>;
  screen: RefObject<HTMLDivElement | null>;
  input: RefObject<HTMLInputElement | null>;
  page: RefObject<HTMLDivElement | null>;
  order: RefObject<HTMLSpanElement | null>;
}

const FIRST_SESSION = 1792700001;
const CALL_HANG = 'calc(7ch + 3ch)';

class Cancelled extends Error {}

const time = (ms: number) => `${(ms / 1000).toFixed(1)}s`.padStart(7, ' ');
const plain = (text: string, cls?: string): Part => (cls ? { text, cls } : { text });

export function useSession(refs: DemoRefs) {
  const [lines, setLines] = useState<Line[]>([]);
  const [input, setInput] = useState('');
  const [busy, setBusy] = useState(false);
  const [closed, setClosed] = useState(false);

  // What the async replay reads lives in refs: it outlives the render that started it.
  const linesRef = useRef<Line[]>([]);
  const nextId = useRef(0);
  const session = useRef(FIRST_SESSION);
  const tasks = useRef(0);
  const busyRef = useRef(false);
  const closedRef = useRef(false);
  const token = useRef(0); // anything newer cancels what is playing
  const touched = useRef(0); // when the visitor last typed: the replay waits for them to stop
  const typing = useRef(false); // the replay is typing into the prompt line
  const reduced = useRef(false);
  const timers = useRef(new Set<number>());

  const later = useCallback((fn: () => void, ms: number) => {
    const id = window.setTimeout(() => {
      timers.current.delete(id);
      fn();
    }, ms);
    timers.current.add(id);
  }, []);

  const sleep = useCallback(
    (ms: number, mine: number) =>
      new Promise<void>((resolve, reject) => {
        later(() => (mine === token.current ? resolve() : reject(new Cancelled())), reduced.current ? Math.min(ms, 1) : ms);
      }),
    [later],
  );

  const { page, drive } = usePage(refs.page, refs.order, later, reduced);
  const hide = useCallback(() => drive('close'), [drive]);

  // One line: plain text, or parts. A blank line separates; at the top of an empty screen there is
  // nothing to separate.
  const line = useCallback((text: string | Part[], kind = '', hang?: string) => {
    if (text === '' && linesRef.current.length === 0) return;
    const parts = typeof text === 'string' ? [plain(text)] : text;
    const next: Line = { id: nextId.current++, kind, parts, ...(hang ? { hang } : {}) };
    linesRef.current = [...linesRef.current, next];
    setLines(linesRef.current);
  }, []);
  const many = useCallback((text: string, kind: string) => text.split('\n').forEach((t) => line(t, kind)), [line]);

  // A new session: an empty screen and the prompt line, nothing else.
  const banner = useCallback(() => {
    session.current += 1;
    tasks.current = 0;
    linesRef.current = [];
    setLines([]);
  }, []);

  const setBusyBoth = useCallback((value: boolean) => {
    busyRef.current = value;
    setBusy(value);
  }, []);
  const setClosedBoth = useCallback((value: boolean) => {
    closedRef.current = value;
    setClosed(value);
  }, []);

  // ---- the recorded run ----

  const replay = useCallback(
    async (mine: number) => {
      banner();
      await sleep(900, mine);
      setInput('');
      typing.current = true;
      let typed = '';
      for (const ch of TASK) {
        await sleep(14 + Math.random() * 22, mine);
        typed += ch;
        setInput(typed);
      }
      await sleep(350, mine);
      typing.current = false;
      setInput('');
      line('');
      line(`› ${TASK}`, 'cmd');
      setBusyBoth(true);
      tasks.current += 1;
      let clock = 0;
      for (const e of EVENTS) {
        await sleep(e.at - clock, mine);
        clock = e.at;
        if (e.call) {
          const [tool, args] = e.call;
          line([plain(time(e.at), 't'), plain(' ▸ '), plain(tool, 'tool'), plain(' '), plain(args, 'arg')], 'call', CALL_HANG);
        }
        if (e.step) line([plain(time(e.at), 't'), plain('   '), plain(e.step)], 'step', CALL_HANG);
        for (const out of e.out ?? []) {
          const [text, kind] = typeof out === 'string' ? [out, undefined] : out;
          line(text, `${kind ?? 'out'} res`);
        }
        for (const [sign, n, code] of e.code ?? []) {
          const kind = sign === '-' ? 'del' : sign === '+' ? 'add' : 'src';
          line([plain(sign, 'sign'), plain(`${String(n).padStart(4, ' ')}  `, 'num'), { code }], `${kind} res`);
        }
        if (e.page) drive(e.page, e.thanks);
      }
      await sleep(END - clock, mine);
      line('');
      SUMMARY.forEach(([text, kind]) => line(text, kind));
      line(`  session  ${session.current}-a3f1`, 'dim');
      line('');
      line(ANSWER, 'answer');
      setBusyBoth(false);
    },
    [banner, drive, line, setBusyBoth, sleep],
  );

  // The visitor takes over: the replay stops where it is.
  const cancel = useCallback(() => {
    token.current += 1;
    typing.current = false;
    setBusyBoth(false);
    hide();
  }, [hide, setBusyBoth]);

  // ---- what a visitor types ----

  const submit = useCallback(
    (text: string) => {
      const entry = text.trim();
      if (closedRef.current) {
        line(`$ ${entry}`, 'cmd');
        if (entry === 'llx' || entry === 'lx') {
          setClosedBoth(false);
          banner();
        } else if (entry) {
          line(`zsh: command not found: ${entry.split(' ')[0]}`, 'dim');
        }
        return;
      }
      line('');
      line(`› ${entry}`, 'cmd');
      const command = entry.split(/\s+/)[0] ?? '';
      switch (command) {
        case '':
          return;
        case '/help':
          return many(HELP, 'dim');
        case '/status':
          return many(
            `model      ${MODEL}\ndecision   ~typesafe/jev-latest\nworkspace  ~/ember\nmode       bypass\nsession    ${session.current}-a3f1 (${tasks.current} tasks)\nterminals  none`,
            'dim',
          );
        case '/new':
        case '/clear':
          return banner();
        case '/exit':
        case '/quit':
          setClosedBoth(true);
          return line('Session ended. Type llx to start again.', 'dim');
        default:
          if (entry.startsWith('/')) return line(`unknown command ${command}: /help lists them`, 'bad');
          line('  This page replays a recorded run; install llx to run your own task.', 'dim');
      }
    },
    [banner, line, many, setClosedBoth],
  );

  const onKeyDown = useCallback(
    (event: KeyboardEvent<HTMLInputElement>) => {
      touched.current = Date.now();
      if (event.key === 'Enter') {
        event.preventDefault();
        if (busyRef.current) cancel();
        const text = event.currentTarget.value;
        setInput('');
        submit(text);
      } else if (event.key === 'd' && event.ctrlKey) {
        event.preventDefault();
        submit('/exit');
      }
    },
    [cancel, submit],
  );

  // Only the visitor's own typing reaches onChange; the replay sets the value through state.
  const onChange = useCallback(
    (event: ChangeEvent<HTMLInputElement>) => {
      touched.current = Date.now();
      cancel();
      setInput(event.target.value);
    },
    [cancel],
  );

  const focus = useCallback(() => refs.input.current?.focus({ preventScroll: true }), [refs.input]);

  // The prompt line follows the output, as in a real terminal.
  useLayoutEffect(() => {
    const screen = refs.screen.current;
    if (screen) screen.scrollTop = screen.scrollHeight;
  }, [lines, refs.screen]);

  // While the replay types, the end of what it typed stays in view.
  useLayoutEffect(() => {
    const field = refs.input.current;
    if (field && typing.current) field.scrollLeft = field.scrollWidth;
  }, [input, refs.input]);

  // Replay, rest, and replay again, once the demo is in view and while the visitor is not typing.
  useEffect(() => {
    reduced.current = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    const pending = timers.current;
    let alive = true;

    const loop = async () => {
      while (alive) {
        if (Date.now() - touched.current > 9000 && !closedRef.current) {
          const mine = ++token.current;
          try {
            await replay(mine);
            await sleep(7000, mine);
          } catch {
            typing.current = false;
            setBusyBoth(false);
            hide();
          }
        } else {
          await new Promise<void>((resolve) => later(resolve, 1000));
        }
      }
    };

    const root = refs.root.current;
    let seen: IntersectionObserver | undefined;
    if (root && 'IntersectionObserver' in window) {
      seen = new IntersectionObserver(
        (entries) => {
          if (entries.some((entry) => entry.isIntersecting)) {
            seen?.disconnect();
            void loop();
          }
        },
        { threshold: 0.3 },
      );
      seen.observe(root);
    } else {
      void loop();
    }

    // Unmounting stops everything: pending sleeps never settle, so the loop goes with them.
    return () => {
      alive = false;
      token.current += 1;
      seen?.disconnect();
      pending.forEach((id) => window.clearTimeout(id));
      pending.clear();
    };
  }, [hide, later, refs.root, replay, setBusyBoth, sleep]);

  return { lines, input, busy, closed, page, onKeyDown, onChange, focus };
}
