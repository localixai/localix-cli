// What a caller's script may use besides the page's own globals (see `script_forms`): `press(key)`
// sends a key the way the `keys` argument does, with the legacy keyCode and which that games read;
// `sleep(ms)` waits. Passed to the script as arguments, so the page never sees them and a script
// may declare names of its own. Past the script's time limit both throw: a loop built on them
// ends, instead of playing on unseen after its answer was given up on.
((deadline) => {
  const due = () => {
    if (performance.now() > deadline) throw new Error('the script ran past its time limit and was stopped')
  }
  return {
    press(key) {
      due()
      const CODES = { Enter: 13, Tab: 9, Escape: 27, ' ': 32, Backspace: 8, ArrowUp: 38, ArrowDown: 40, ArrowLeft: 37, ArrowRight: 39 }
      const legacy = CODES[key] ?? (key.length === 1 ? key.toUpperCase().charCodeAt(0) : 0)
      const code = /^[a-z]$/i.test(key) ? `Key${key.toUpperCase()}` : /^[0-9]$/.test(key) ? `Digit${key}` : key === ' ' ? 'Space' : key
      const active = document.activeElement
      const target = active && active !== document.documentElement ? active : document.body
      for (const type of ['keydown', 'keyup']) {
        const event = new KeyboardEvent(type, { key, code, bubbles: true, cancelable: true, composed: true })
        Object.defineProperty(event, 'keyCode', { get: () => legacy })
        Object.defineProperty(event, 'which', { get: () => legacy })
        target.dispatchEvent(event)
      }
    },
    async sleep(ms) {
      due()
      await new Promise((resolve) => setTimeout(resolve, ms))
      due()
    },
  }
})(performance.now() + __SCRIPT_MS__)
