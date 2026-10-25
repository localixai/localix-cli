import type { Register } from 'claude-code'

// The llx servers a window's tabs may belong to: this plugin's own, or one added by hand
// (`claude mcp add llx`). Comments the person sends from the window come back with the next
// `window` call, so a quiet `list` every few seconds brings them in as a prompt.
const SERVERS = ['plugin:llx:llx', 'llx']
// How the window tool marks what the person sent, after a call's own report.
const MARK = '### What the person sent from the window'
// What the window says the person did there (a tab closed, a page moved) is for the agent's next
// call; only a batch of comments is worth a prompt of its own.
const COMMENTS = 'The person sent '
const EVERY_MS = 2000

// One poll at a time: a slow daemon never stacks calls.
let polling = false

export const register: Register = on => {
  on('session.start', async ($, e, next) => {
    $.clock.every(EVERY_MS, async () => {
      if (polling) return
      polling = true
      try {
        for (const server of SERVERS) {
          let text = ''
          try {
            const result = await $.mcp.call(server, 'window', { action: 'list' })
            if (result.isError) continue
            text = result.content.map(block => (block.type === 'text' ? block.text : '')).join('\n')
          } catch {
            continue
          }
          const at = text.indexOf(MARK)
          if (at < 0) continue
          const sent = text.slice(at)
          if (!sent.includes(COMMENTS)) continue
          $.ui.toast('comments from the llx window')
          void $.prompt.submit({
            text: `The person sent comments from the llx window (${server}). Act on them.\n\n${sent}`,
          })
        }
      } finally {
        polling = false
      }
    })
    return next(e)
  })
}
