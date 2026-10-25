import './Agents.css';

const COMMANDS: [string, string][] = [
  ['Claude Code', 'claude mcp add llx -- llx mcp'],
  ['Codex', 'codex mcp add llx -- llx mcp'],
  ['Any MCP client', 'llx mcp'],
];

/** llx's window for other agents: the same tool llx has, served over MCP. */
export function Agents() {
  return (
    <section className="agents wrap" aria-labelledby="agents-title">
      <h2 id="agents-title">A window for your other agents.</h2>
      <p>
        <code>llx mcp</code> gives Claude Code, Codex, Cursor or any agent that speaks MCP llx&rsquo;s <code>window</code>: real
        browser pages and terminal sessions as tabs of one window you watch. Their model thinks; llx&rsquo;s small decision
        model carries out a tab&rsquo;s goal.
      </p>
      <dl>
        {COMMANDS.map(([name, command]) => (
          <div key={name}>
            <dt>{name}</dt>
            <dd>
              <code>{command}</code>
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
