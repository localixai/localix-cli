// What the plugin reads off llx's output. llx names a build `<version>+<size:x>.<mtime secs:x>`
// of its own executable (apps/cli/src/daemon.rs, `build`): the binary on disk and the daemon are
// current when they are one build.

/** What `llx serve status` says of the daemon. */
export type Daemon =
  | { isRunning: false }
  | {
      isRunning: true
      pid: number
      build: string
      attached: number
      lanes: number
      working: number
      kept: number
      jobs: number
    }

/** The build llx would name itself, from `llx --version` and its executable's stat. */
export function buildOf(version: string, size: number, mtimeMs: number): string | undefined {
  const number = /^llx\s+(\S+)/.exec(version.trim())?.[1]
  if (number === undefined || size <= 0 || mtimeMs <= 0) return undefined
  return `${number}+${size.toString(16)}.${Math.floor(mtimeMs / 1000).toString(16)}`
}

const STATUS =
  /^llx serve: running, pid (\d+), build (\S+), (\d+) attached, (\d+) sessions? open, (\d+) working, (\d+) in the background, (\d+) jobs?/

/** `llx serve status`'s line read, or undefined when it says something else. */
export function daemonFrom(stdout: string): Daemon | undefined {
  const line = stdout.trim()
  if (line.startsWith('llx serve: not running')) return { isRunning: false }
  const found = STATUS.exec(line)
  if (!found) return undefined
  const [pid, build, attached, lanes, working, kept, jobs] = found.slice(1)
  const n = (text: string | undefined) => Number(text ?? 0)
  return {
    isRunning: true,
    pid: n(pid),
    build: build ?? '',
    attached: n(attached),
    lanes: n(lanes),
    working: n(working),
    kept: n(kept),
    jobs: n(jobs),
  }
}

/** Whether stopping the daemon takes nothing from anyone but the clients named `ours`. */
export function isIdle(daemon: Daemon, ours: number): boolean {
  if (!daemon.isRunning) return true
  const others = Math.max(0, daemon.attached - ours)
  return others + daemon.lanes + daemon.working + daemon.kept + daemon.jobs === 0
}

/** What keeps a daemon busy, in words. */
export function busyWith(daemon: Daemon, ours: number): string {
  if (!daemon.isRunning) return 'nothing'
  const parts: string[] = []
  const say = (count: number, one: string, many: string) => {
    if (count > 0) parts.push(`${count} ${count === 1 ? one : many}`)
  }
  say(Math.max(0, daemon.attached - ours), 'other client', 'other clients')
  say(daemon.lanes, 'session open', 'sessions open')
  say(daemon.working, 'task working', 'tasks working')
  say(daemon.kept, 'in the background', 'in the background')
  say(daemon.jobs, 'job', 'jobs')
  return parts.join(', ') || 'nothing'
}

/** What the window's `list` says: how many tabs are open, or undefined when it cannot tell. */
export function tabsIn(listed: string): number | undefined {
  if (/^No tabs are open/m.test(listed)) return 0
  const found = /^(\d+) tabs?:/m.exec(listed)
  return found ? Number(found[1]) : undefined
}

/**
 * What a shell command does with `llx window`: a call (the daemon has the owner's tabs now), the
 * owner's end, or neither.
 */
export function windowUse(command: string): 'call' | 'end' | undefined {
  if (!/(^|[\s;&|(`])llx\s+window\b/.test(command)) return undefined
  return /\bllx\s+window\s+(--as\s+\S+\s+)?--end\b/.test(command) ? 'end' : 'call'
}
