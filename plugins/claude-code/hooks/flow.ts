import type { EngineInterface } from 'claude-code'

type $ = EngineInterface

/** The checkout `/llx rebuild` installs from: the one this session works in. */
export async function sourceOf($: $): Promise<string | undefined> {
  const at = await $.session.cwd()
  try {
    const root = await $.process.run(
      ['cargo', 'locate-project', '--workspace', '--message-format', 'plain'],
      { cwd: at, stdin: '' },
    )
    if (root.exitCode !== 0) return undefined
    const dir = root.stdout.trim().replace(/\/Cargo\.toml$/, '')
    const app = await $.fs.read(`${dir}/apps/cli/Cargo.toml`)
    return /^name\s*=\s*"llx"/m.test(app) ? dir : undefined
  } catch {
    return undefined
  }
}

/** `cargo install` of the checkout at `dir`, its progress on the status line; the tail of what it said when it failed. */
export async function install($: $, dir: string): Promise<{ isInstalled: boolean; tail: string }> {
  const said: string[] = []
  let pending = ''
  const child = $.process.spawn({ argv: ['cargo', 'install', '--locked', '--path', 'apps/cli'], cwd: dir })
  let ended: { code: number | null } = { code: null }
  const stream = child[Symbol.asyncIterator]()
  for (;;) {
    const piece = await stream.next()
    if (piece.done) {
      ended = piece.value
      break
    }
    pending += piece.value.text
    const lines = pending.split('\n')
    pending = lines.pop() ?? ''
    for (const line of lines) {
      const shown = line.trim()
      if (!shown) continue
      said.push(shown)
      if (said.length > 40) said.shift()
      $.ui.status(`llx: building · ${shown.slice(0, 60)}`)
    }
  }
  if (pending.trim()) said.push(pending.trim())
  return { isInstalled: ended.code === 0, tail: said.slice(-15).join('\n') }
}
