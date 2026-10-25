export const GITHUB = 'https://github.com/rsaryev/llx';
/** The install, then each agent's MCP setup: one tab each. */
export const INSTALL = [
  ['llx', 'npm install -g @rsaryev/llx'],
  ['Claude Code', 'claude mcp add llx -- npx -y @rsaryev/llx mcp'],
  ['Codex', 'codex mcp add llx -- npx -y @rsaryev/llx mcp'],
] as const;
