import type { ReactNode } from 'react';

// A small highlighter for the HTML and JavaScript the run shows: tags, strings, keywords, numbers,
// comments and punctuation. Enough to find the change at a glance; not a parser.
const TOKENS =
  /(<\/?[a-z!][^\s>]*|>)|('(?:[^'\\]|\\.)*'|"(?:[^"\\]|\\.)*"|`[^`]*`)|(\/\/.*$)|\b(const|let|var|function|return|if|else|for|while|new|class|await|async|true|false|null|undefined)\b|\b(\d+(?:\.\d+)?)\b|([()[\]{}.,;=>])/g;

export function highlight(code: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  let last = 0;
  for (const m of code.matchAll(TOKENS)) {
    if (m.index > last) nodes.push(code.slice(last, m.index));
    const kind = m[1] ? 't' : m[2] ? 's' : m[3] ? 'c' : m[4] ? 'k' : m[5] ? 'n' : 'p';
    nodes.push(
      <span key={m.index} className={`tk-${kind}`}>
        {m[0]}
      </span>,
    );
    last = m.index + m[0].length;
  }
  if (last < code.length) nodes.push(code.slice(last));
  return nodes;
}
