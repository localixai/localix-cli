import { useEffect, useRef, useState } from 'react';

/** One beat of the window's scene: which project and tab are shown, what is on them, and the line
 *  under the window that says who is acting. */
export type Beat = {
  project: 'trips' | 'shop';
  tab: 'flights' | 'term' | 'page';
  say: string;
  from: string;
  to: string;
  results: boolean;
  picked: boolean;
  booked: boolean;
  lines: number;
  commenting: boolean;
  target: 'none' | 'button' | 'total';
  card: boolean;
  note: string;
  marks: number;
  sent: boolean;
  ms: number;
};

const FLIGHT = 'agent · goal: book the cheapest Zürich → London flight on Friday';
const TESTS = 'agent · terminal: npm test, in the same window';
const YOU = 'you · comment on the page, then send';

const start: Beat = {
  project: 'trips', tab: 'flights', say: FLIGHT, from: '', to: '', results: false, picked: false, booked: false, lines: 0,
  commenting: false, target: 'none', card: false, note: '', marks: 0, sent: false, ms: 1000,
};

/** `text` typed into `key` of `from`, two letters a beat. */
function typing(from: Beat, key: 'from' | 'to' | 'note', text: string): Beat[] {
  const beats: Beat[] = [];
  for (let n = 2; n < text.length + 2; n += 2) beats.push({ ...from, [key]: text.slice(0, n), ms: 80 } as Beat);
  return beats;
}

/** A comment on `target`: hovered, its card typed in, then a mark; and the beat it ends on. */
function comment(from: Beat, target: 'button' | 'total', note: string): [Beat[], Beat] {
  const hover: Beat = { ...from, target, ms: 600 };
  const card: Beat = { ...hover, card: true, ms: 400 };
  const end: Beat = { ...from, marks: from.marks + 1, ms: 700 };
  return [[hover, card, ...typing(card, 'note', note), { ...card, note, ms: 500 }, end], end];
}

function scene(): Beat[] {
  const from: Beat = { ...start, from: 'Zürich' };
  const both: Beat = { ...from, to: 'London' };
  const found: Beat = { ...both, results: true, ms: 900 };
  const terminal: Beat = { ...found, project: 'shop', tab: 'term', say: TESTS, lines: 1, ms: 700 };
  const page: Beat = { ...terminal, tab: 'page', say: YOU, commenting: true, ms: 700 };
  const [first, one] = comment(page, 'button', 'Full width on a phone');
  const [second, two] = comment(one, 'total', 'Show the price with tax');
  return [
    start,
    ...typing(start, 'from', 'Zürich'),
    ...typing(from, 'to', 'London'),
    { ...both, ms: 500 },
    found,
    { ...found, picked: true, ms: 800 },
    { ...found, picked: true, booked: true, ms: 1600 },
    terminal,
    { ...terminal, lines: 2, ms: 250 },
    { ...terminal, lines: 3, ms: 250 },
    { ...terminal, lines: 4, ms: 250 },
    { ...terminal, lines: 5, ms: 1500 },
    page,
    ...first,
    ...second,
    { ...two, ms: 900 },
    { ...two, marks: 0, commenting: false, sent: true, say: 'you · sent: the agent fixes both', ms: 2400 },
  ];
}

const BEATS = scene();

/** The beat now: the scene plays in a loop once the window is in view, and stands still, at its
 *  most telling beat, for a visitor who asks for less motion. */
export function useScene(root: React.RefObject<HTMLElement | null>): Beat {
  const [at, setAt] = useState(0);
  const playing = useRef(false);

  useEffect(() => {
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
      setAt(BEATS.length - 3);
      return;
    }
    let timer: number | undefined;
    const next = (from: number) => {
      timer = window.setTimeout(() => {
        const to = (from + 1) % BEATS.length;
        setAt(to);
        next(to);
      }, BEATS[from]?.ms ?? 1000);
    };
    const seen = new IntersectionObserver(
      (entries) => {
        if (!playing.current && entries.some((entry) => entry.isIntersecting)) {
          playing.current = true;
          next(0);
        }
      },
      { threshold: 0.3 },
    );
    if (root.current) seen.observe(root.current);
    return () => {
      seen.disconnect();
      window.clearTimeout(timer);
    };
  }, [root]);

  return BEATS[at] ?? start;
}
