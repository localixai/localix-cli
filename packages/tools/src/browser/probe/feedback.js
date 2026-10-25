  // Visual feedback: while the person comments, the
  // page takes no presses. A click picks the element under the pointer, a drag marks a region and
  // the elements inside it, and a card anchored to it takes an optional note. What was picked goes
  // to the host as data ({fb}), where the agent reads it as what the person did in the window.
  // Everything is drawn in a closed shadow root the snapshot never sees.
  const FB_MAX_TEXT = 300
  const FB_MAX_MEMBERS = 8
  const FB_DRAG = 6
  const FB_STYLES = ['display', 'position', 'color', 'background-color', 'font-size', 'font-weight', 'padding', 'margin', 'border-radius']
  const fb = { on: false, all: false, sent: 0, root: null, hover: null, card: null, drag: null, start: null, marks: [], count: 0 }

  const fbClip = (s, n = FB_MAX_TEXT) => {
    const t = String(s || '').replace(/\s+/g, ' ').trim()
    return t.length > n ? `${t.slice(0, n)}…` : t
  }
  const fbBox = (r) => ({ x: Math.round(r.left), y: Math.round(r.top), width: Math.round(r.width), height: Math.round(r.height) })
  const fbOurs = (node) => fb.root !== null && node instanceof Node && (node === fb.root.host || fb.root.contains(node))

  // A stable selector: the nearest id, then each step's tag and place among its own kind.
  function fbSelector(e) {
    const steps = []
    for (let n = e; n && n.nodeType === 1 && n !== document.documentElement; n = n.parentElement) {
      if (n.id && !/\d{4,}/.test(n.id)) {
        steps.unshift(`#${CSS.escape(n.id)}`)
        break
      }
      const same = n.parentElement ? [...n.parentElement.children].filter((c) => c.tagName === n.tagName) : []
      const tag = n.tagName.toLowerCase()
      steps.unshift(same.length > 1 ? `${tag}:nth-of-type(${same.indexOf(n) + 1})` : tag)
    }
    return steps.join(' > ')
  }

  // How a person would name the way down: tags with their first class, at most six deep.
  function fbPath(e) {
    const steps = []
    for (let n = e; n && n.nodeType === 1 && steps.length < 6; n = n.parentElement) {
      const cls = typeof n.className === 'string' ? n.className.trim().split(/\s+/)[0] : ''
      steps.unshift(n.tagName.toLowerCase() + (n.id ? `#${n.id}` : cls ? `.${cls}` : ''))
    }
    return steps.join(' > ')
  }

  // Where in the source the element was written, when the page says: a stamped attribute, else
  // React's development fiber. Best effort; nothing when neither is there.
  function fbSource(e) {
    for (let n = e; n && n.nodeType === 1; n = n.parentElement) {
      const loc = n.getAttribute('data-source-loc') || n.getAttribute('data-inspector-relative-path') || n.getAttribute('data-source-file')
      if (loc) return { file: loc, line: n.getAttribute('data-inspector-line') || n.getAttribute('data-source-line') || undefined }
    }
    const key = Object.keys(e).find((k) => k.startsWith('__reactFiber$') || k.startsWith('__reactInternalInstance$'))
    let components = []
    for (let f = key ? e[key] : null, i = 0; f && i < 40; f = f.return, i++) {
      const name = typeof f.type === 'function' ? f.type.displayName || f.type.name : null
      if (name && components.length < 4) components.push(name)
      const s = f._debugSource || f.memoizedProps?.__source
      if (s?.fileName) return { file: s.fileName, line: s.lineNumber, column: s.columnNumber, components }
    }
    return components.length ? { components } : undefined
  }

  function fbStyles(e) {
    const style = getComputedStyle(e)
    const kept = {}
    for (const name of FB_STYLES) kept[name] = style.getPropertyValue(name)
    return kept
  }

  function fbElement(e) {
    const html = e.outerHTML || ''
    return {
      tag: html.slice(0, html.indexOf('>') + 1).slice(0, 200),
      role: e.getAttribute('role') || undefined,
      name: fbClip(e.getAttribute('aria-label') || e.getAttribute('title') || e.getAttribute('alt') || '', 120) || undefined,
      text: fbClip(e.innerText || e.textContent),
      selector: fbSelector(e),
      path: fbPath(e),
      box: fbBox(e.getBoundingClientRect()),
      styles: fbStyles(e),
      source: fbSource(e),
    }
  }

  // The leaves inside a region: elements wholly in it whose parent is not.
  function fbMembers(r) {
    const inside = (b) => b.width > 0 && b.left >= r.left && b.top >= r.top && b.right <= r.left + r.width && b.bottom <= r.top + r.height
    const all = [...document.body.querySelectorAll('*')].filter((e) => !fbOurs(e) && inside(e.getBoundingClientRect()))
    const leaves = all.filter((e) => !all.some((o) => o !== e && e.contains(o)))
    return leaves.slice(0, FB_MAX_MEMBERS).map((e) => ({ selector: fbSelector(e), text: fbClip(e.innerText || e.textContent, 80) }))
  }

  function fbMount() {
    if (fb.root) return
    const host = document.createElement('llx-feedback')
    host.setAttribute('aria-hidden', 'true')
    host.style.cssText = 'all:initial;position:fixed;inset:0;z-index:2147483647;pointer-events:none'
    fb.root = host.attachShadow({ mode: 'closed' })
    fb.root.innerHTML = `<style>
.o{position:fixed;box-sizing:border-box;border:2px solid #4f7cff;border-radius:3px;background:#4f7cff14;pointer-events:none;display:none}
.d{border-style:dashed}
.m{position:fixed;box-sizing:border-box;border-radius:3px;background:#ffb02033;outline:1px solid #ffb020aa;pointer-events:none}
.m b{position:absolute;top:-9px;left:-9px;min-width:18px;height:18px;border-radius:9px;background:#ffb020;color:#000;font:600 11px/18px system-ui;text-align:center}
.c{position:fixed;width:280px;padding:8px;border-radius:10px;background:#1d1d1f;color:#f2f2f2;font:13px system-ui;box-shadow:0 8px 28px #0006;pointer-events:auto}
.c textarea{box-sizing:border-box;width:100%;height:64px;resize:none;border:0;border-radius:6px;padding:6px;font:inherit;background:#2c2c2e;color:inherit;outline:none}
.c div{display:flex;justify-content:flex-end;gap:6px;margin-top:6px}
.c button{border:0;border-radius:6px;padding:4px 10px;font:inherit;background:#3a3a3c;color:inherit;cursor:pointer}
.c button.s{background:#4f7cff;color:#fff}
</style><div class="o"></div><div class="o d"></div>`
    ;[fb.hover, fb.drag] = fb.root.querySelectorAll('.o')
    document.documentElement.append(host)
  }

  const fbShow = (box, r) => Object.assign(box.style, { display: 'block', left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`, height: `${r.height}px` })

  // The marks of comments sent, measured again as the page moves.
  function fbMeasure() {
    for (const mark of fb.marks) {
      const r = mark.target ? mark.target.getBoundingClientRect() : mark.rect
      if (mark.target && !mark.target.isConnected) mark.box.style.display = 'none'
      else fbShow(mark.box, r)
    }
  }

  // The card closes, and the outline of what it was about with it.
  function fbClose() {
    fb.card?.remove()
    fb.card = null
    fb.hover.style.display = fb.drag.style.display = 'none'
  }

  // The card for one capture, beside what it is about: Enter sends, Escape drops it.
  function fbOpen(capture, rect, target) {
    fbClose()
    const card = document.createElement('div')
    card.className = 'c'
    card.innerHTML = '<textarea placeholder="What should change? (optional)"></textarea><div><button>Cancel</button><button class="s">Send</button></div>'
    const [cancel, send] = card.querySelectorAll('button')
    const note = card.querySelector('textarea')
    const left = Math.min(Math.max(8, rect.left), innerWidth - 296)
    const below = rect.bottom + 120 < innerHeight
    Object.assign(card.style, { left: `${left}px`, top: `${below ? rect.bottom + 8 : Math.max(8, rect.top - 120)}px` })
    // The card and outline go first, and the mark comes later: the host takes the picture of
    // what was commented on from the screen, which must show the page alone.
    const submit = () => {
      const said = note.value.trim()
      fbClose()
      post(JSON.stringify({ t: TOKEN, fb: { ...capture, note: said, url: location.href, n: ++fb.count } }))
      const n = fb.count
      setTimeout(() => {
        if (n > fb.count) return
        const box = document.createElement('div')
        box.className = 'm'
        box.innerHTML = `<b>${n}</b>`
        fb.root.append(box)
        fb.marks.push({ box, target, rect })
        fbMeasure()
      }, 800)
    }
    send.addEventListener('click', submit)
    cancel.addEventListener('click', fbClose)
    note.addEventListener('keydown', (event) => {
      event.stopPropagation()
      if (event.key === 'Enter' && !event.shiftKey) {
        event.preventDefault()
        submit()
      } else if (event.key === 'Escape') fbClose()
    })
    fb.root.append(card)
    fb.card = card
    note.focus({ preventScroll: true })
  }

  const fbAt = (x, y) => {
    const e = document.elementFromPoint(x, y)
    return e && !fbOurs(e) && e !== document.documentElement ? e : null
  }

  // Presses on the page stop here while commenting: a control under the pointer is picked, not
  // operated. The card's own events pass.
  // Only the person's own presses (trusted): the page's scripts and the agent's actions go on
  // as ever. `all` takes synthetic ones too, for checks.
  function fbSwallow(event) {
    if (!fb.on || (!event.isTrusted && !fb.all) || event.composedPath().some(fbOurs)) return false
    event.preventDefault()
    event.stopImmediatePropagation()
    return true
  }

  function fbDown(event) {
    if (!fbSwallow(event)) return
    fb.start = { x: event.clientX, y: event.clientY }
  }

  function fbMove(event) {
    if (!fb.on || fb.card || (!event.isTrusted && !fb.all)) return
    const s = fb.start
    if (s && Math.hypot(event.clientX - s.x, event.clientY - s.y) > FB_DRAG) {
      fb.hover.style.display = 'none'
      const left = Math.min(s.x, event.clientX)
      const top = Math.min(s.y, event.clientY)
      return fbShow(fb.drag, { left, top, width: Math.abs(event.clientX - s.x), height: Math.abs(event.clientY - s.y) })
    }
    const e = fbAt(event.clientX, event.clientY)
    if (e) fbShow(fb.hover, e.getBoundingClientRect())
  }

  function fbUp(event) {
    if (!fbSwallow(event)) return
    const s = fb.start
    fb.start = null
    fb.drag.style.display = fb.hover.style.display = 'none'
    if (!s) return
    const width = Math.abs(event.clientX - s.x)
    const height = Math.abs(event.clientY - s.y)
    if (Math.hypot(width, height) > FB_DRAG) {
      const rect = { left: Math.min(s.x, event.clientX), top: Math.min(s.y, event.clientY), width, height }
      rect.right = rect.left + width
      rect.bottom = rect.top + height
      fbOpen({ region: fbBox(rect), members: fbMembers(rect) }, rect, null)
      return fbShow(fb.drag, rect)
    }
    const e = fbAt(event.clientX, event.clientY)
    if (!e) return
    fbOpen({ element: fbElement(e) }, e.getBoundingClientRect(), e)
    fbShow(fb.hover, e.getBoundingClientRect())
  }

  for (const type of ['click', 'dblclick', 'auxclick', 'contextmenu', 'touchstart', 'touchend', 'submit']) {
    addEventListener(type, fbSwallow, { capture: true })
  }
  addEventListener('pointerdown', fbDown, { capture: true })
  addEventListener('mousedown', fbSwallow, { capture: true })
  addEventListener('mouseup', fbSwallow, { capture: true })
  addEventListener('pointerup', fbUp, { capture: true })
  addEventListener('pointermove', fbMove, { capture: true, passive: true })
  addEventListener('scroll', () => fb.marks.length && requestAnimationFrame(fbMeasure), { capture: true, passive: true })
  addEventListener('resize', () => fb.marks.length && requestAnimationFrame(fbMeasure))

  // The host turns commenting on or off; the marks of comments held stay until a batch is sent.
  function comment({ on, sent = fb.sent, all = fb.all }) {
    fb.on = Boolean(on)
    fb.all = Boolean(all)
    if (sent !== fb.sent) {
      fb.sent = sent
      for (const mark of fb.marks) mark.box.remove()
      fb.marks = []
      fb.count = 0
    }
    if (fb.on) fbMount()
    else if (fb.root) fbClose()
    if (fb.root) fb.root.host.style.cursor = fb.on ? 'crosshair' : ''
    document.documentElement.style.cursor = fb.on ? 'crosshair' : ''
    return fb.on
  }
