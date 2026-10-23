import { useCallback, useState } from 'react';
import type { RefObject } from 'react';
import type { PageAction } from './recording';

/** The page beside the terminal: whether it is shown, the cursor, the button and the thanks line. */
export interface PageState {
  on: boolean;
  cursorOn: boolean;
  cursorClick: boolean;
  cursorTransform: string;
  pressed: boolean;
  thanks: boolean;
}

const CLOSED: PageState = {
  on: false,
  cursorOn: false,
  cursorClick: false,
  cursorTransform: 'translate(70%, 30%)',
  pressed: false,
  thanks: false,
};

/**
 * Drives the page card. A click moves the cursor onto the Order button (measured, so it lands on
 * the button at any width), then presses it; `later` schedules on the session's timers.
 */
export function usePage(
  frame: RefObject<HTMLElement | null>,
  order: RefObject<HTMLElement | null>,
  later: (fn: () => void, ms: number) => void,
  reduced: RefObject<boolean>,
) {
  const [page, setPage] = useState<PageState>(CLOSED);

  const drive = useCallback(
    (what: PageAction, thanks = false) => {
      if (what === 'open') {
        setPage({ ...CLOSED, on: true });
      } else if (what === 'close') {
        setPage((p) => ({ ...p, on: false }));
      } else {
        const outer = frame.current?.getBoundingClientRect();
        const box = order.current?.getBoundingClientRect();
        if (!outer || !box) return;
        const x = box.left - outer.left + box.width * 0.55;
        const y = box.top - outer.top + box.height * 0.55;
        setPage((p) => ({ ...p, cursorOn: true, cursorClick: false, cursorTransform: `translate(${x}px, ${y}px)` }));
        later(
          () => setPage((p) => ({ ...p, cursorClick: true, pressed: true, thanks: p.thanks || thanks })),
          reduced.current ? 0 : 300,
        );
        later(() => setPage((p) => ({ ...p, pressed: false })), 460);
      }
    },
    [frame, later, order, reduced],
  );

  return { page, drive };
}
