  // `evaluate`: the caller's script runs in the page realm and its value comes back
  // as compact text (see `script_forms` in webview.rs for how it is started).
  const MAX_EVAL = 8000
  function serialize(v) {
    const cap = (s) => (s.length > MAX_EVAL ? `${s.slice(0, MAX_EVAL)}… [truncated, ${s.length} chars]` : s)
    if (v === undefined) return 'undefined'
    if (v === null || ['number', 'boolean', 'bigint'].includes(typeof v)) return String(v)
    if (typeof v === 'string') return cap(v)
    if (v instanceof Element) return cap(`<${v.tagName.toLowerCase()}> ${(v.textContent || '').trim().replace(/\s+/g, ' ')}`)
    if (v instanceof NodeList || v instanceof HTMLCollection) return cap([...v].map(serialize).join('\n'))
    try {
      const seen = new WeakSet()
      return cap(JSON.stringify(v, (_k, x) => {
        if (typeof x === 'object' && x !== null) {
          if (seen.has(x)) return '[Circular]'
          seen.add(x)
        }
        return typeof x === 'bigint' ? String(x) : x
      }, 2) ?? '[unserializable]')
    } catch {
      return '[unserializable]'
    }
  }
  // The caller's script, already started by the native side (which a page's CSP cannot block),
  // hands its promise here; its value comes back over IPC like any other answer.
  async function settled(running) {
    leaving = false
    try {
      return { result: serialize(await running) }
    } catch (error) {
      throw new Error(`the script threw: ${error?.message || error}`)
    }
  }

  const ready = () => document.readyState
  const CALLS = { snapshot, act, settle, read, settled, ready, keys, locate }
  Object.defineProperty(window, '__dloop_call', {
    value(id, name, args) {
      const reply = (body) => post(JSON.stringify({ t: TOKEN, id, ...body }))
      Promise.resolve()
        .then(() => CALLS[name](args))
        .then((ok) => reply({ ok: ok ?? null }), (error) => reply({ err: String(error?.message || error) }))
      return true
    },
  })
})()
