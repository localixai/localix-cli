// The page side of the browser tool (five files, concatenated into one script by host.rs): an atomic snapshot of what a person can see and use, and the execution
// of one action on an element that snapshot observed. Installed before any page script runs, so the
// token and the IPC post captured here cannot be replaced by the page. No caller reaches this file
// with a selector or code: an action names an observed element by its ref.
;(() => {
  if (window.top !== window || window.__dloop_call) return
  const TOKEN = '__TOKEN__'
  // An invisible window nobody can see must not take what a person types elsewhere: real
  // (trusted) keys and text input are dropped. The probe's own events are synthetic and pass.
  if (__HEADLESS__) {
    for (const type of ['keydown', 'keypress', 'keyup', 'beforeinput', 'input', 'paste', 'compositionstart']) {
      addEventListener(type, (event) => {
        if (!event.isTrusted) return
        event.stopImmediatePropagation()
        event.preventDefault()
      }, { capture: true })
    }
  }
  const post = window.ipc.postMessage.bind(window.ipc)
  const MAX_ELEMENTS = 250
  const MAX_TEXT = 3000

  const ids = new WeakMap()
  const nodes = new Map()
  let next = 1
  const refOf = (e) => {
    if (!ids.has(e)) ids.set(e, next++)
    const id = ids.get(e)
    nodes.set(id, e)
    return 'e' + id
  }
  const nodeOf = (ref) => nodes.get(Number(String(ref).slice(1)))
  const epoch = () => String(performance.timeOrigin)

  // Whether this document is being replaced: a link, a form or a script started a navigation of
  // the main frame. Seen here first; the native side only hears of it once the server answers.
  let leaving = false
  addEventListener('beforeunload', () => { leaving = true }, { capture: true })
  window.navigation?.addEventListener('navigate', (event) => {
    if (!event.destination?.sameDocument && !event.hashChange && !event.downloadRequest) leaving = true
  })
  // Known at the moment of the action, before any of the above: a link click or a form submit
  // the page did not cancel will replace the document (a new tab opens in this window, too).
  // Listening on window, registered first, these run after the page's own handlers.
  addEventListener('click', (event) => {
    const link = event.target?.closest?.('a[href]')
    if (!link || event.defaultPrevented || link.hasAttribute('download')) return
    const to = new URL(link.href, location.href)
    if (!/^https?:$/.test(to.protocol)) return
    if (to.href.split('#')[0] !== location.href.split('#')[0]) leaving = true
  })
  addEventListener('submit', (event) => {
    if (!event.defaultPrevented) leaving = true
  })

  // A transparent checkbox or radio is a styled control (its picture is a sibling), not a hidden one.
  const visible = (e) =>
    !e.closest('[aria-hidden="true"],[inert]') &&
    e.checkVisibility({ checkOpacity: !/^(checkbox|radio)$/.test(e.type), checkVisibilityCSS: true })
  // A password field is offered for typing, but its value is never read back.
  const secret = (e) => ['file', 'hidden'].includes(e.type)

  const ROLES = ['button', 'link', 'checkbox', 'radio', 'switch', 'tab', 'menuitem',
    'menuitemradio', 'menuitemcheckbox', 'option', 'gridcell', 'combobox', 'textbox', 'searchbox',
    'spinbutton', 'slider', 'treeitem']
  const SELECTOR = 'a[href],button,input,textarea,select,summary,[contenteditable="true"],' +
    ROLES.map((role) => `[role="${role}"]`).join(',')

  const roleOf = (e) => {
    const explicit = e.getAttribute('role')
    if (ROLES.includes(explicit)) return explicit
    const tag = e.tagName
    if (tag === 'BUTTON' || tag === 'SUMMARY') return 'button'
    if (tag === 'A') return 'link'
    if (tag === 'SELECT') return 'combobox'
    if (tag === 'TEXTAREA' || e.isContentEditable) return 'textbox'
    if (tag !== 'INPUT') return null
    if (e.type === 'checkbox' || e.type === 'radio') return e.type
    if (['button', 'submit', 'reset', 'image'].includes(e.type)) return 'button'
    if (e.type === 'search') return 'searchbox'
    if (e.type === 'number') return 'spinbutton'
    if (e.type === 'range') return 'slider'
    if (e.type === 'password') return 'textbox'
    return 'textbox'
  }

  // Accessible name, in the order the spec uses.
  const nameOf = (e, seen = new Set()) => {
    if (!e || seen.has(e)) return ''
    seen.add(e)
    const labelled = (e.getAttribute('aria-labelledby') || '').split(/\s+/).filter(Boolean)
      .map((id) => nameOf(e.ownerDocument.getElementById(id), seen)).filter(Boolean).join(' ')
    const own = labelled || e.getAttribute('aria-label') ||
      [...(e.labels || [])].map((label) => label.innerText.trim()).filter(Boolean).join(' ') ||
      // An input button reads its value; a <button>'s value is data for the form, not its name.
      (e.tagName === 'INPUT' && ['button', 'submit', 'reset'].includes(e.type) ? e.value : '') ||
      e.getAttribute('alt') ||
      (['INPUT', 'TEXTAREA', 'SELECT'].includes(e.tagName) ? '' : (e.innerText || '').trim()) ||
      e.getAttribute('placeholder') || e.getAttribute('title') || e.getAttribute('name') || ''
    return own.replace(/\s+/g, ' ').trim().slice(0, 120)
  }

  // A nameless control (a row's checkbox, an icon button) is known by the text of its row.
  // Or, for a bare checkbox followed by its words (`<input type=checkbox> checkbox 1<br>`), by the
  // text right after it, else right before it.
  const contextOf = (e) => {
    const row = e.closest('li,tr,[role="row"],[role="listitem"],label,fieldset')
    if (row) return (row.innerText || '').replace(/\s+/g, ' ').trim().slice(0, 80)
    const beside = (node, step) => {
      for (let n = node[step]; n; n = n[step]) {
        if (n.nodeType === 1 && (n.matches(SELECTOR) || n.tagName === 'BR')) return ''
        const text = (n.textContent || '').replace(/\s+/g, ' ').trim()
        if (text) return text.slice(0, 80)
      }
      return ''
    }
    return beside(e, 'nextSibling') || beside(e, 'previousSibling')
  }

  // A calendar day reads "15" in every month a picker shows. Name it with its date when the page
  // says it (data-iso, data-date, datetime), else with its month's heading: "15 (2026-11-15)".
  const ISO = /\d{4}-\d{2}-\d{2}/
  const MONTH = /(jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec|янв|фев|мар|апр|мая|май|июн|июл|авг|сен|окт|ноя|дек)/i
  const dayOf = (e, name) => {
    if (!/^\s*\d{1,2}(\D|$)/.test(name) || name.length > 40) return name
    for (let n = e; n && n !== document.body; n = n.parentElement) {
      for (const key of ['data-iso', 'data-date', 'datetime', 'data-day', 'data-value']) {
        const date = n.getAttribute?.(key)?.match(ISO)
        if (date) return `${name} (${date[0]})`
      }
      const group = n.matches('[role="rowgroup"],[role="grid"],table')
      const heading = group && ((n.getAttribute('aria-label') || '') ||
        (n.querySelector('caption,[role="heading"],h1,h2,h3,h4,h5,h6')?.innerText || '') ||
        (n.innerText || '').split('\n')[0])
      if (heading && MONTH.test(heading) && heading.length < 40) return `${name} (${heading.trim()})`
    }
    return name
  }

  const editableOf = (e, role) =>
    !e.readOnly && e.getAttribute('aria-readonly') !== 'true' &&
    (['textbox', 'searchbox', 'spinbutton'].includes(role) ||
      (role === 'combobox' && ['INPUT', 'TEXTAREA'].includes(e.tagName)) || e.isContentEditable)

  const valueOf = (e, role) => {
    if (e.type === 'password') return e.value ? '••••••' : ''
    if (e.type === 'checkbox' || e.type === 'radio') return ''
    if (e.tagName === 'SELECT') return [...e.selectedOptions].map((o) => o.label).join(', ')
    if ('value' in e && e.tagName !== 'BUTTON' && e.tagName !== 'LI') return String(e.value)
    if (e.isContentEditable || role === 'combobox') return (e.innerText || '').trim().slice(0, 120)
    return ''
  }

  const statesOf = (e) => {
    const states = []
    if (e.matches(':disabled') || e.closest('[aria-disabled="true"]')) states.push('disabled')
    const expanded = e.getAttribute('aria-expanded')
    if (expanded) states.push(expanded === 'true' ? 'expanded' : 'collapsed')
    if (e.checked === true || e.getAttribute('aria-checked') === 'true') states.push('checked')
    if (e.getAttribute('aria-selected') === 'true' || e.getAttribute('aria-current')) states.push('selected')
    if (e.required || e.getAttribute('aria-required') === 'true') states.push('required')
    if (e.readOnly || e.getAttribute('aria-readonly') === 'true') states.push('readonly')
    return states
  }

  const inViewport = (r) =>
    r.width > 0 && r.height > 0 && r.bottom > 0 && r.right > 0 &&
    r.top < innerHeight && r.left < innerWidth
  // Up to one screen below the fold: a control there (a cookie banner's Accept, a form's Submit)
  // is clicked directly, scrolled into view by the click, instead of costing a scroll first.
  const justBelow = (r) =>
    r.width > 0 && r.height > 0 && r.right > 0 && r.left < innerWidth &&
    r.top >= innerHeight && r.top < innerHeight * 2

  const DIALOG = 'dialog[open],[role="dialog"],[role="alertdialog"],[aria-modal="true"]'

  function snapshot() {
    if (!document.body) return null
    for (const [id, e] of nodes) if (!e.isConnected || !e.ownerDocument.defaultView) nodes.delete(id)
    const elements = []
    let omitted = 0
    const frames = subframes()
    // An overlay frame first: it is what the page is about now, and the element cap must not cut it.
    const docs = [...frames.filter((f) => f.overlay), { doc: document, frame: null }, ...frames.filter((f) => !f.overlay)]
    for (const { doc, frame, ...at } of docs) for (const e of doc.querySelectorAll(SELECTOR)) {
      if (secret(e) || !visible(e)) continue
      const role = roleOf(e)
      const own = e.getBoundingClientRect()
      const box = shifted(own, frame && at)
      // Inside a frame, only what its own viewport shows.
      if (frame && (own.bottom <= 0 || own.right <= 0 || own.top >= frame.clientHeight || own.left >= frame.clientWidth)) continue
      if (!role || !(inViewport(box) || justBelow(box))) continue
      if (role === 'gridcell' && e.querySelector('button,[role="button"]')) continue
      if (elements.length >= MAX_ELEMENTS) { omitted++; continue }
      const element = { ref: refOf(e), role, name: dayOf(e, nameOf(e) || contextOf(e)) }
      const value = valueOf(e, role)
      if (value) element.value = value.slice(0, 200)
      const states = statesOf(e)
      if (!inViewport(box)) states.push('below')
      if (states.length) element.states = states
      if (editableOf(e, role)) element.editable = true
      if (e.closest(DIALOG) || at.overlay) element.dialog = true
      if (e.tagName === 'SELECT') {
        element.options = [...e.options].filter((o) => !o.disabled).slice(0, 50)
          .map((o) => ({ value: o.value, label: o.label }))
      }
      elements.push(element)
    }
    return {
      epoch: epoch(),
      url: location.href,
      title: document.title,
      text: [...frames.filter((f) => f.overlay).map((f) => visibleText(f.doc, f)), visibleText()].join('\n').slice(0, MAX_TEXT),
      outline: [...frames.filter((f) => f.overlay).map((f) => outline(f.doc, f)), outline()].filter(Boolean).join('\n'),
      viewport: { w: innerWidth, h: innerHeight },
      scroll: { y: Math.round(scrollY), height: document.documentElement.scrollHeight },
      elements,
      omitted,
      busy: busy(),
    }
  }

  // Whether the page says it is still working: a document still parsing or fetching, a busy
  // region, a progress bar, or words such as "Loading…" on screen.
  // The page's own requests in flight (fetch and XHR to its origin), by start time: results a
  // search asked for arrive this way, after the DOM has gone quiet. Third-party traffic (ads,
  // analytics) never ends and is not the page's content; a request older than INFLIGHT_MAX is a
  // long poll.
  const INFLIGHT_MAX = 5000
  const inflight = new Map()
  let requests = 0
  const own = (url) => { try { return new URL(url, location.href).origin === location.origin } catch { return false } }
  const track = (url, promise) => {
    if (!own(url)) return
    const id = ++requests
    inflight.set(id, performance.now())
    const done = () => inflight.delete(id)
    promise.then(done, done)
  }
  const nativeFetch = window.fetch
  if (nativeFetch) {
    window.fetch = function (input, init) {
      const result = nativeFetch.call(this, input, init)
      track(input?.url ?? String(input), result)
      return result
    }
  }
  const nativeOpen = XMLHttpRequest.prototype.open
  const nativeSend = XMLHttpRequest.prototype.send
  const urls = new WeakMap()
  XMLHttpRequest.prototype.open = function (method, url, ...rest) {
    urls.set(this, String(url))
    return nativeOpen.call(this, method, url, ...rest)
  }
  XMLHttpRequest.prototype.send = function (body) {
    track(urls.get(this) || '', new Promise((resolve) => this.addEventListener('loadend', resolve, { once: true })))
    return nativeSend.call(this, body)
  }
  const fetching = () => [...inflight.values()].some((start) => performance.now() - start < INFLIGHT_MAX)

  function busy() {
    // Still parsing, or fetching its own data. Images, videos and ad frames still loading into a
    // parsed page are not content the goal waits for.
    if (document.readyState === 'loading' || fetching()) return true
    const shown = (selector) => [...document.querySelectorAll(selector)]
      .some((e) => visible(e) && inViewport(e.getBoundingClientRect()))
    return shown('[aria-busy="true"],[role="progressbar"],progress') ||
      /\b(loading|please wait|searching)\b/i.test(visibleText().slice(0, 2000))
  }

  // The words a person can read in the viewport; off-screen bodies and footers stay out.
  function visibleText(doc = document, frame = null) {
    const words = []
    const walker = doc.createTreeWalker(doc.body, NodeFilter.SHOW_TEXT)
    const range = doc.createRange()
    let length = 0
    for (let node = walker.nextNode(); node && length < MAX_TEXT; node = walker.nextNode()) {
      const value = node.textContent.replace(/\s+/g, ' ').trim()
      const parent = node.parentElement
      if (!value || !parent || parent.closest('script,style,noscript,template') || !visible(parent)) continue
      range.selectNodeContents(node)
      if (!inViewport(shifted(range.getBoundingClientRect(), frame))) continue
      words.push(value)
      length += value.length + 1
    }
    return words.join('\n').slice(0, MAX_TEXT)
  }

