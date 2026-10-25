export const GITHUB = 'https://github.com/rsaryev/llx';
/** The install, then each agent's MCP setup: one tab each. */
export const INSTALL = [
  ['llx', 'npm install -g @localixai/llx'],
  ['Claude Code', 'claude mcp add llx -- npx -y @localixai/llx mcp'],
  ['Codex', 'codex mcp add llx -- npx -y @localixai/llx mcp'],
] as const;
