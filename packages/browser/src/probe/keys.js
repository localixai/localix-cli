  // Keys: a chord such as "ArrowUp", "Enter", "Ctrl+A" or "a" becomes
  // keydown (and keypress for a character) then keyup on the focused element, carrying `key`,
  // `code` and the legacy `keyCode`/`which` that games read. Synthetic keys run no default action,
  // so Tab moves focus and Enter submits a form here.
  const NAMED_KEYS = {
    enter: 'Enter', return: 'Enter', tab: 'Tab', escape: 'Escape', esc: 'Escape', space: ' ',
    spacebar: ' ', backspace: 'Backspace', delete: 'Delete', del: 'Delete', arrowup: 'ArrowUp',
    up: 'ArrowUp', arrowdown: 'ArrowDown', down: 'ArrowDown', arrowleft: 'ArrowLeft',
    left: 'ArrowLeft', arrowright: 'ArrowRight', right: 'ArrowRight', home: 'Home', end: 'End',
    pageup: 'PageUp', pagedown: 'PageDown',
  }
  const KEY_CODES = {
    Enter: 13, Tab: 9, Escape: 27, ' ': 32, Backspace: 8, Delete: 46, ArrowUp: 38, ArrowDown: 40,
    ArrowLeft: 37, ArrowRight: 39, Home: 36, End: 35, PageUp: 33, PageDown: 34,
  }
  const physicalCode = (key) => {
    if (/^[a-z]$/i.test(key)) return `Key${key.toUpperCase()}`
    if (/^[0-9]$/.test(key)) return `Digit${key}`
    if (key === ' ') return 'Space'
    return key.length > 1 ? key : ''
  }

  function chord(text) {
    const parts = String(text).trim().split('+').map((p) => p.trim())
    let key = parts.pop() || (String(text).endsWith('+') ? '+' : '')
    const mods = new Set(parts.map((m) => m.toLowerCase()))
    key = NAMED_KEYS[key.toLowerCase()] ?? key
    return {
      key, code: physicalCode(key), bubbles: true, cancelable: true, composed: true,
      ctrlKey: mods.has('ctrl') || mods.has('control'), metaKey: mods.has('meta') || mods.has('cmd'),
      altKey: mods.has('alt'), shiftKey: mods.has('shift'),
    }
  }

  function press(text) {
    const init = chord(text)
    const legacy = KEY_CODES[init.key] ?? (init.key.length === 1 ? init.key.toUpperCase().charCodeAt(0) : 0)
    const target = document.activeElement && document.activeElement !== document.documentElement
      ? document.activeElement : document.body
    const fire = (type) => {
      const event = new KeyboardEvent(type, init)
      Object.defineProperty(event, 'keyCode', { get: () => legacy })
      Object.defineProperty(event, 'which', { get: () => legacy })
      return target.dispatchEvent(event)
    }
    const proceed = fire('keydown')
    if (proceed && init.key.length === 1 && !init.ctrlKey && !init.metaKey) fire('keypress')
    fire('keyup')
    if (proceed && init.key === 'Enter' && target.form) target.form.requestSubmit()
    if (proceed && init.key === 'Tab') {
      const order = [...document.querySelectorAll('a[href],button,input,select,textarea,[tabindex]')]
        .filter((e) => e.tabIndex >= 0 && !e.disabled && visible(e))
      const at = order.indexOf(target)
      order[at + (init.shiftKey ? -1 : 1)]?.focus()
    }
  }

  // A sequence of keys, `delay` ms apart so a game's animation can take each one.
  async function keys(args) {
    leaving = false
    const list = (args.keys || []).slice(0, 500)
    for (const [i, key] of list.entries()) {
      press(key)
      if (i < list.length - 1) await new Promise((r) => setTimeout(r, args.delay ?? 60))
    }
    return { pressed: list.length }
  }

