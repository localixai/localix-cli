import { shellTokens } from '../components/Shell';

export function escapeHtml(text: string): string {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

const span = (kind: string, text: string) =>
  kind === 'plain' ? escapeHtml(text) : `<span class="tk-${kind}">${escapeHtml(text)}</span>`;

/** A code block as HTML, coloured as the landing's terminals (syntax.css): a shell's commands,
 *  flags, strings and comments; YAML's and JSON's keys and strings. Anything else as it is. */
export function highlight(code: string, lang: string): string {
  const lines = code.replace(/\n$/, '').split('\n');
  if (lang === 'bash' || lang === 'sh' || lang === 'shell') return lines.map(shell).join('\n');
  if (lang === 'yaml' || lang === 'yml') return yaml(lines);
  if (lang === 'json') return lines.map(json).join('\n');
  return escapeHtml(lines.join('\n'));
}

function shell(line: string, at: number, lines: string[]): string {
  const continued = at > 0 && (lines[at - 1] ?? '').endsWith('\\');
  return shellTokens(line, continued)
    .map(([kind, text]) => span(kind, text))
    .join('');
}

/** A value: its quoted strings, and a comment after it (` #`, outside quotes). */
function value(text: string): string {
  return text
    .split(/('[^']*'|"[^"]*"|\s#.*$)/)
    .map((part) => span(/^['"]/.test(part) ? 'str' : /^\s#/.test(part) ? 'comment' : 'plain', part))
    .join('');
}

/** YAML line by line. A block scalar (`key: |`) keeps its lines as written: a prompt, a script or
 *  a schema there is text, and a `#` in it is not a comment. */
function yaml(lines: string[]): string {
  let block = -1;
  return lines
    .map((line) => {
      const indent = line.length - line.trimStart().length;
      if (block >= 0 && (line.trim() === '' || indent > block)) return escapeHtml(line);
      block = -1;
      if (line.trimStart().startsWith('#')) return span('comment', line);
      const item = /^(\s*(?:-\s+)?)(?:([A-Za-z0-9_.-]+)(:))?(.*)$/.exec(line);
      const lead = item?.[1] ?? '';
      const rest = item?.[4] ?? '';
      if (/^\s*[|>][-+]?\s*$/.test(rest)) block = item?.[2] ? lead.length : indent;
      const key = item?.[2] ? span('flag', item[2]) + (item[3] ?? '') : '';
      return escapeHtml(lead) + key + value(rest);
    })
    .join('\n');
}

function json(line: string): string {
  return line.replace(/("(?:[^"\\]|\\.)*")(\s*:)?|[^"]+/g, (all, string?: string, colon?: string) =>
    string ? span(colon ? 'flag' : 'str', string) + (colon ?? '') : escapeHtml(all),
  );
}
