  // The element on top at the point, through open shadow roots.
  function topAt(x, y) {
    let top = document.elementFromPoint(x, y)
    while (top?.shadowRoot) {
      const deeper = top.shadowRoot.elementFromPoint(x, y)
      if (!deeper || deeper === top) break
      top = deeper
    }
    return top
  }

  // Resolve a ref for input: the same document, still attached, visible, enabled and on top.
  function target(args) {
    if (args.epoch !== epoch()) throw new Error('stale: the page changed since the snapshot')
    const e = nodeOf(args.ref)
    if (!e || !e.isConnected || !visible(e)) throw new Error('stale: the element is gone')
    if (e.matches(':disabled') || e.closest('[aria-disabled="true"]')) throw new Error('disabled')
    e.scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' })
    const r = e.getBoundingClientRect()
    const x = Math.max(1, Math.min(r.left + r.width / 2, innerWidth - 1))
    const y = Math.max(1, Math.min(r.top + r.height / 2, innerHeight - 1))
    const top = topAt(x, y)
    const label = top?.tagName === 'LABEL' && top.control === e
    if (top && !label && top !== e && !e.contains(top) && !top.contains(e)) {
      throw new Error(`covered: ${top.tagName.toLowerCase()} "${nameOf(top)}" is on top`)
    }
    return { e, x, y, hit: top && e.contains(top) ? top : e }
  }

  function mouse(el, type, x, y) {
    const down = type.endsWith('down')
    const init = { bubbles: true, cancelable: true, composed: true, view: window, clientX: x,
      clientY: y, button: 0, buttons: down ? 1 : 0, detail: type.startsWith('pointer') ? 0 : 1 }
    el.dispatchEvent(type.startsWith('pointer')
      ? new PointerEvent(type, { ...init, pointerId: 1, isPrimary: true, pointerType: 'mouse' })
      : new MouseEvent(type, init))
  }

  function click({ e, x, y, hit }) {
    for (const type of ['pointerover', 'mouseover', 'pointerdown', 'mousedown']) mouse(hit, type, x, y)
    e.focus?.({ preventScroll: true })
    for (const type of ['pointerup', 'mouseup', 'click']) mouse(hit, type, x, y)
  }

  // The native setter, so React and Vue value trackers see the change.
  function fill(e, text) {
    e.focus?.({ preventScroll: true })
    if (e instanceof HTMLInputElement || e instanceof HTMLTextAreaElement) {
      const proto = e instanceof HTMLTextAreaElement ? HTMLTextAreaElement : HTMLInputElement
      Object.getOwnPropertyDescriptor(proto.prototype, 'value').set.call(e, text)
      e.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText', data: text }))
      e.dispatchEvent(new Event('change', { bubbles: true }))
    } else if (e.isContentEditable) {
      e.textContent = text
      e.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText', data: text }))
    } else {
      throw new Error('not_editable')
    }
  }

  // Synthetic keys run no default action, so Enter submits the form itself unless a handler took it.
  function enter(e) {
    const init = { key: 'Enter', code: 'Enter', bubbles: true, cancelable: true, composed: true }
    const key = (type) => {
      const event = new KeyboardEvent(type, init)
      Object.defineProperty(event, 'keyCode', { get: () => 13 })
      Object.defineProperty(event, 'which', { get: () => 13 })
      return e.dispatchEvent(event)
    }
    const proceed = key('keydown') && key('keypress')
    key('keyup')
    if (proceed && e.form) e.form.requestSubmit()
  }

  function act(args) {
    leaving = false
    switch (args.op) {
      case 'scroll_down':
      case 'scroll_up':
        scrollBy({ top: (args.op === 'scroll_down' ? 0.8 : -0.8) * innerHeight, behavior: 'instant' })
        return { ok: true }
      case 'press_enter': {
        const e = args.ref ? target(args).e : document.activeElement
        if (!e || e === document.body) throw new Error('no focused field to press Enter in')
        enter(e)
        return { ok: true }
      }
      case 'click':
        click(target(args))
        return { ok: true }
      case 'type':
        fill(target(args).e, String(args.value ?? ''))
        return { ok: true }
      case 'select': {
        const { e } = target(args)
        if (e.tagName !== 'SELECT') throw new Error('not a dropdown')
        const option = [...e.options].find((o) => o.value === args.value && !o.disabled)
        if (!option) throw new Error('no such option')
        // Picking the option already chosen fires nothing in a browser; pages rely on that.
        if (option.selected) return { ok: true, unchanged: true }
        e.focus?.({ preventScroll: true })
        e.value = option.value
        e.dispatchEvent(new Event('input', { bubbles: true }))
        e.dispatchEvent(new Event('change', { bubbles: true }))
        return { ok: true }
      }
      default:
        throw new Error('unknown operation ' + args.op)
    }
  }

  // After an action: until the DOM has been quiet for 100 ms (at least 30 ms), at most 700 ms;
  // after typing into a combobox, until its options show (suggestions come from the network: up
  // to 1.5 s, and only as long as none are shown). A page that unloads ends it. Polled on a
  // timer: animation frames stop in a window the system considers in the background.
  function settle(args) {
    const field = args.ref ? nodeOf(args.ref) : null
    const suggesting = args.op === 'type' && /combobox|searchbox/.test(field?.getAttribute('role') || field?.type || '')
    return new Promise((resolve) => {
      const start = performance.now()
      let last = start
      let done = false
      const observer = new MutationObserver(() => { last = performance.now() })
      observer.observe(document, { subtree: true, childList: true, attributes: true, characterData: true })
      const finish = () => {
        if (done) return
        done = true
        observer.disconnect()
        resolve({ ok: true, navigating: leaving, ms: Math.round(performance.now() - start) })
      }
      addEventListener('pagehide', finish, { once: true })
      const poll = () => {
        if (done) return
        const now = performance.now()
        const shown = !suggesting || [...document.querySelectorAll('[role="option"]')]
          .some((o) => visible(o) && inViewport(o.getBoundingClientRect()))
        if ((now - start >= 30 && now - last >= 100 && shown) || now - start >= (suggesting ? 1500 : 700)) finish()
        else setTimeout(poll, 15)
      }
      setTimeout(poll, 15)
    })
  }

  // Animation frames in half a second: whether this window still renders (the headless check).
  function frames() {
    return new Promise((resolve) => {
      let count = 0
      const start = performance.now()
      const tick = () => {
        count++
        if (performance.now() - start < 500) requestAnimationFrame(tick)
        else resolve({ frames: count, visibility: document.visibilityState })
      }
      requestAnimationFrame(tick)
      setTimeout(() => resolve({ frames: count, visibility: document.visibilityState }), 1500)
    })
  }

  // The whole document's readable text, a window of it at a time.
  function read(args) {
    const text = (document.body?.innerText || '').replace(/[ \t]+/g, ' ').replace(/\n{3,}/g, '\n\n')
    const offset = Math.max(0, args?.offset || 0)
    return { text: text.slice(offset, offset + (args?.limit || 6000)), total: text.length, offset }
  }

