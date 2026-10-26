import type { EngineInterface } from 'claude-code'
import { buildOf, daemonFrom, type Daemon } from './build'

type $ = EngineInterface

// The last look at the binary: it is stat'ed on each look, and its version asked only when the
// file changed.
let last: { path: string; size: number; mtimeMs: number; build?: string } | undefined

/** Where `llx` resolves on the PATH, and the build that file is. */
export async function disk($: $): Promise<{ path?: string; build?: string }> {
  try {
    let path = last?.path
    if (!path) {
      const found = await $.process.run(['sh', '-c', 'command -v llx'], { stdin: '' })
      path = found.stdout.trim()
      if (found.exitCode !== 0 || !path) return {}
    }
    // An npm install puts a link on the PATH: the build is of the file it leads to.
    const stat = await $.fs.stat(path, { resolve: true })
    if (last?.path === path && last.size === stat.size && last.mtimeMs === stat.mtimeMs) return last
    const version = await $.process.run([path, '--version'], { stdin: '' })
    last = { path, size: stat.size, mtimeMs: stat.mtimeMs, build: buildOf(version.stdout, stat.size, stat.mtimeMs) }
    return last
  } catch {
    last = undefined
    return {}
  }
}

/** The daemon, as `llx serve status` says; undefined when it cannot be asked. */
export async function daemon($: $): Promise<Daemon | undefined> {
  try {
    const ran = await $.process.run(['llx', 'serve', 'status'], { stdin: '', timeoutMs: 15000 })
    return daemonFrom(ran.stdout)
  } catch {
    return undefined
  }
}

/**
 * `llx window ARGS` run in `cwd`, as the agent's shell runs it there: the same owner (the
 * project) and so the same tabs. What it printed, and whether it was done.
 */
export async function window($: $, cwd: string, args: string[]): Promise<{ isDone: boolean; text: string }> {
  try {
    const ran = await $.process.run(['llx', 'window', ...args], { cwd, stdin: '', timeoutMs: 20000 })
    return { isDone: ran.exitCode === 0, text: [ran.stdout.trim(), ran.stderr.trim()].filter(Boolean).join('\n') }
  } catch (error) {
    return { isDone: false, text: String(error) }
  }
}
