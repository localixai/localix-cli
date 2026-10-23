import { useEffect, useRef, useState } from 'react';
import './InstallCommand.css';

type Label = 'Copy' | 'Copied' | 'Selected';

/** The install command, with Copy as its own segment. Where the clipboard is refused, the command
 *  is selected instead, so the visitor can copy it themselves. */
export function InstallCommand({ command }: { command: string }) {
  const text = useRef<HTMLSpanElement>(null);
  const [label, setLabel] = useState<Label>('Copy');
  const reset = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(reset.current), []);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(command);
      setLabel('Copied');
    } catch {
      const node = text.current;
      const selection = window.getSelection();
      if (node && selection) {
        const range = document.createRange();
        range.selectNodeContents(node);
        selection.removeAllRanges();
        selection.addRange(range);
      }
      setLabel('Selected');
    }
    window.clearTimeout(reset.current);
    reset.current = window.setTimeout(() => setLabel('Copy'), 1600);
  };

  return (
    <div className="command">
      <code>
        <span className="sigil" aria-hidden="true">
          $
        </span>{' '}
        <span ref={text}>{command}</span>
      </code>
      <button type="button" onClick={copy} data-done={label === 'Copy' ? undefined : ''}>
        <span className="copy-label">{label}</span>
      </button>
    </div>
  );
}
