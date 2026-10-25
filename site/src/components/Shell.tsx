/** One token of a shell line, and how it reads. */
type Kind = 'cmd' | 'flag' | 'str' | 'op' | 'plain' | 'comment';

const TOKEN = /(#.*$|'[^']*'|"[^"]*"|\s+|\\$|\||>|[^\s|>]+)/g;

/** A shell line split into tokens: the first word of a command, flags, strings, pipes. */
function tokens(line: string, continued: boolean): [Kind, string][] {
  let first = !continued || line.startsWith('|');
  return (line.match(TOKEN) ?? []).map((text): [Kind, string] => {
    if (/^\s+$/.test(text)) return ['plain', text];
    if (text.startsWith('#')) return ['comment', text];
    if (text === '|' || text === '>' || text === '\\') {
      first = text === '|';
      return ['op', text];
    }
    if (text.startsWith("'") || text.startsWith('"')) return ['str', text];
    if (text.startsWith('-')) return ['flag', text];
    if (first) {
      first = false;
      return ['cmd', text];
    }
    return ['plain', text];
  });
}

/** A terminal session: `$ ` lines are commands, coloured, and so are the lines a `\\` continues;
 *  the rest is their output, dimmed. */
export function Terminal({ session, label }: { session: string; label: string }) {
  const lines = session.split('\n');
  return (
    <pre className="term" aria-label={label}>
      {lines.map((line, at) => {
        const continued = at > 0 && (lines[at - 1] ?? '').endsWith('\\');
        if (!line.startsWith('$ ') && !continued) {
          return (
            <span key={at} className={line ? 'code-line term-out' : 'code-line term-gap'}>
              {line || ' '}
            </span>
          );
        }
        return (
          <span key={at} className="code-line">
            <span className="tk-prompt">{continued ? '  ' : '$ '}</span>
            {tokens(continued ? line.trimStart() : line.slice(2), continued).map(([kind, text], n) => (
              <span key={n} className={`tk-${kind}`}>
                {text}
              </span>
            ))}
          </span>
        );
      })}
    </pre>
  );
}
