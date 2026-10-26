import type { Register } from 'claude-code'
import { busyWith, isIdle, tabsIn, windowUse } from './build'
import { promptFor, sentFrom } from './comments'
import { install, sourceOf } from './flow'
import { daemon, disk, window } from './host'

const POLL_MS = 2000
const LIST = '{"action":"list"}'

// Whether this session's project may have tabs open: set by a `llx window` call, cleared by its
// end or a list with none. Only then is the window asked: a poll never starts a daemon.
let watching = false
// Whether this session called `llx window` at all: its tabs end with it.
let used = false
// One poll at a time: a slow daemon never stacks calls.
let polling = false
let building = false

export const register: Register = on => {
  on('session.start', async ($, e, next) => {
    const started = await next(e)
    // A refused name (a skill of the same name still listed) costs the command, not the relay.
    await $.command
      .register({
        name: 'llx',
        description: 'llx: status, doctor, rebuild, end (this project\'s window tabs)',
        argumentHint: '[status|doctor|rebuild|end]',
      })
      .catch(error => $.ui.log(`llx plugin: /llx not registered: ${String(error)}`, { to: 'debug' }))
    // Tabs an earlier session of this project left open: their comments come in too.
    const served = await daemon($)
    if (served?.isRunning && served.kept > 0) watching = true

    $.clock.every(POLL_MS, async () => {
      if (!watching || polling) return
      polling = true
      try {
        const listed = await window($, await $.session.cwd(), [LIST])
        if (!listed.isDone) return
        const sent = sentFrom(listed.text)
        if (sent !== undefined) {
          $.ui.toast('comments from the llx window')
          void $.prompt.submit({ text: promptFor(sent) })
        } else if (tabsIn(listed.text) === 0) watching = false
      } finally {
        polling = false
      }
    })
    return started
  })

  // The agent calls `llx window` through Bash: the plugin only notices, to relay the comments.
  on('tool.call', { tool: 'Bash' }, async ($, e, next) => {
    const result = await next(e)
    const use = windowUse(e.command)
    if (use === 'call') {
      watching = true
      used = true
    } else if (use === 'end') watching = false
    return result
  }).catch(($, e, next) => next(e))

  on('session.end', async ($, e, next) => {
    if (used) await window($, await $.session.cwd(), ['--end'])
    return next(e)
  }).catch(($, e, next) => next(e))

  on('command.run', { command: 'llx' }, async ($, e) => {
    const [verb = 'status'] = e.args.trim().split(/\s+/)
    const cwd = await $.session.cwd()
    if (verb === 'end') {
      const ended = await window($, cwd, ['--end'])
      watching = false
      return { text: ended.isDone ? "This project's llx tabs are closed." : `llx window --end: ${ended.text}` }
    }
    if (verb === 'rebuild') {
      if (building) return { text: 'llx is already building.' }
      const dir = await sourceOf($)
      if (!dir) return { text: 'This session is not in a checkout of llx: /llx rebuild builds llx from its sources, for its developers. To update llx, run npm update -g @localixai/llx.' }
      building = true
      void (async () => {
        try {
          const built = await install($, dir)
          if (built.isInstalled) {
            $.ui.log(`llx rebuilt from ${dir}. The next llx call starts the new build once the daemon is idle (/llx end closes this project's tabs).`)
            $.ui.toast('llx rebuilt')
          } else {
            $.ui.log(`llx rebuild failed in ${dir}:\n${built.tail}`)
            $.ui.toast('llx rebuild failed')
          }
        } catch (error) {
          $.ui.log(`llx rebuild failed: ${String(error)}`)
        } finally {
          building = false
          $.ui.status(undefined)
        }
      })()
      return { text: `Building llx from ${dir} (cargo install --locked --path apps/cli); progress on the status line.` }
    }
    if (verb === 'doctor') {
      const ran = await $.process.run(['llx', 'doctor'], { stdin: '', timeoutMs: 60000 }).catch(error => ({ stdout: '', stderr: String(error), exitCode: 1 }))
      return { text: [ran.stdout.trim(), ran.stderr.trim()].filter(Boolean).join('\n') || `llx doctor: exit ${ran.exitCode}` }
    }
    const [found, served] = await Promise.all([disk($), daemon($)])
    const lines = [`binary: ${found.path ?? 'not on the PATH'}${found.build ? ` (build ${found.build})` : ''}`]
    if (!served) lines.push('daemon: could not be asked')
    else if (!served.isRunning) lines.push('daemon: not running (the first llx window call starts it)')
    else {
      const current = served.build === found.build
      const old = isIdle(served, 0) ? 'OLD: the next call replaces it' : `OLD, in use (${busyWith(served, 0)}): /llx end, or llx serve stop`
      lines.push(`daemon: pid ${served.pid}, build ${served.build} (${current ? 'current' : old}), ${served.attached} attached, ${served.working} working, ${served.kept} in the background`)
      const listed = await window($, cwd, [LIST])
      lines.push(`window (this project):\n${listed.text}`)
    }
    return { text: lines.join('\n') }
  })
}
