import { expect, test } from 'claude-code/testing'
import { buildOf, busyWith, daemonFrom, isIdle, tabsIn, windowUse } from './build'
import { promptFor, sentFrom } from './comments'

const REPORT = '2 tabs open.\n\n### What the person sent from the window\n\nThe person sent 1 comment on tab 2: "make it red"'

test('a report with comments becomes one prompt; one without, none', async () => {
  const sent = sentFrom(REPORT)
  expect(sent?.startsWith('### What the person sent')).toBe(true)
  expect(promptFor(sent ?? '')).toContain('make it red')
  expect(sentFrom('2 tabs open.')).toBe(undefined)
  // The person closed a tab: for the agent's next call, not a turn of its own.
  expect(sentFrom('### What the person sent from the window\n\nClosed tab 3.')).toBe(undefined)
})

test("the agent's shell commands are read for its use of the window", async () => {
  expect(windowUse(`llx window '{"action":"open","kind":"browser","url":"http://localhost:3000"}'`)).toBe('call')
  expect(windowUse(`cd app && llx window - <<'EOF'\n{}\nEOF`)).toBe('call')
  expect(windowUse('llx window --end')).toBe('end')
  expect(windowUse('llx window --as review --end')).toBe('end')
  expect(windowUse('llx exec "open a window"')).toBe(undefined)
  expect(windowUse('grep -r "llx window" docs')).toBe(undefined)
})

test('the build is named as llx names it', async () => {
  // The daemon of the old binary said `build 0.1.0+395f20.6add5867` for 3759904 bytes.
  expect(buildOf('llx 0.1.0\n', 3759904, 0x6add5867 * 1000 + 999)).toBe('0.1.0+395f20.6add5867')
  expect(buildOf('', 1, 1)).toBe(undefined)
})

test('the daemon status line is read, and who keeps it busy', async () => {
  const line =
    'llx serve: running, pid 20747, build 0.1.0+395f20.6add5867, 1 attached, 0 sessions open, 0 working, 0 in the background, 0 jobs\n'
  const found = daemonFrom(line)
  expect(found).toEqual({ isRunning: true, pid: 20747, build: '0.1.0+395f20.6add5867', attached: 1, lanes: 0, working: 0, kept: 0, jobs: 0 })
  expect(daemonFrom('llx serve: not running\n')).toEqual({ isRunning: false })
  expect(daemonFrom('garbage')).toBe(undefined)
  if (!found?.isRunning) throw new Error('unread')
  expect(isIdle(found, 1)).toBe(true)
  expect(isIdle(found, 0)).toBe(false)
  expect(busyWith({ ...found, attached: 2, working: 1 }, 1)).toBe('1 other client, 1 task working')
})

test('the tab count is read off the window list', async () => {
  expect(tabsIn('No tabs are open.')).toBe(0)
  expect(tabsIn('2 tabs:\nw1  browser ...')).toBe(2)
  expect(tabsIn('1 tab:\nw1  browser ...')).toBe(1)
  expect(tabsIn('something else')).toBe(undefined)
})
