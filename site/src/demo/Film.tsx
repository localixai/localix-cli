import { useEffect, useRef } from 'react';
import 'asciinema-player/dist/bundle/asciinema-player.css';
import './Film.css';

/**
 * A real llx session, recorded with asciinema (scripts/record-llx.exp) and replayed as text, so
 * it stays crisp and a few kilobytes. The waits are trimmed in the file itself
 * (scripts/polish-cast.py); the player loads in the browser only.
 */
export function Film({ src, label }: { src: string; label: string }) {
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let player: { dispose: () => void } | undefined;
    let alive = true;
    void import('asciinema-player').then((AsciinemaPlayer) => {
      if (!alive || !root.current) return;
      player = AsciinemaPlayer.create(src, root.current, {
        autoPlay: true,
        loop: true,
        preload: true,
        fit: 'width',
        controls: 'auto',
        theme: 'llx',
        terminalFontFamily: 'Mono, ui-monospace, monospace',
      });
    });
    return () => {
      alive = false;
      player?.dispose();
    };
  }, [src]);

  return <div ref={root} className="film" aria-label={label} />;
}
