import '../styles/syntax.css';

/** One token of a shell line, and how it reads. */
type Kind = 'cmd' | 'flag' | 'str' | 'op' | 'plain' | 'comment';

const TOKEN = /(#.*$|'[^']*'|"[^"]*"|\s+|\\$|\||>|[^\s|>]+)/g;

/** A shell line split into tokens: the first word of a command, flags, strings, pipes (the docs'
 *  code blocks use it too). */
export function shellTokens(line: string, continued: boolean): [Kind, string][] {
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

/** A line of a workflow: `key:` and `- key:` at its start, quoted strings; the rest as it is. */
function workflow(line: string) {
  const key = /^(\s*(?:-\s+)?)([A-Za-z0-9_.-]+)(:)(.*)$/.exec(line);
  const indent = key?.[1] ?? '';
  const name = key?.[2] ?? '';
  const colon = key?.[3] ?? '';
  const value = key ? (key[4] ?? '') : line;
  const parts = value.split(/('[^']*'|"[^"]*")/).filter(Boolean);
  return (
    <>
      <span>{indent}</span>
      <span className="tk-flag">{name}</span>
      <span>{colon}</span>
      {parts.map((part, n) => (
        <span key={n} className={/^['"]/.test(part) ? 'tk-str' : undefined}>
          {part}
        </span>
      ))}
    </>
  );
}

/** A terminal session: `$ ` lines are commands, coloured, and so are the lines a `\\` continues;
 *  the rest is their output, dimmed. With `yaml`, the rest is a workflow's YAML (a `#` line is a
 *  comment) and `= ` marks an output. */
export function Terminal({ session, label, yaml = false }: { session: string; label: string; yaml?: boolean }) {
  const lines = session.split('\n');
  return (
    <pre className="term" aria-label={label}>
      {lines.map((line, at) => {
        const continued = at > 0 && (lines[at - 1] ?? '').endsWith('\\');
        if (yaml && !line.startsWith('$ ') && !continued) {
          if (line.startsWith('= ')) {
            return (
              <span key={at} className="code-line term-out">
                {line.slice(2)}
              </span>
            );
          }
          const comment = line.trimStart().startsWith('#');
          return (
            <span key={at} className={comment ? 'code-line tk-comment' : 'code-line'}>
              {comment ? line : workflow(line)}
            </span>
          );
        }
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
            {shellTokens(continued ? line.trimStart() : line.slice(2), continued).map(([kind, text], n) => (
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
