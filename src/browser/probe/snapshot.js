// The page side of lxsurf (three files, concatenated into one script by host.rs): an atomic snapshot of what a person can see and use, and the execution
// of one action on an element that snapshot observed. Installed before any page script runs, so the
// token and the IPC post captured here cannot be replaced by the page. Model output never reaches
// this file as a selector or code: an action names an observed element by its ref.
;(() => {
  if (window.top !== window || window.__lxsurf_call) return
  const TOKEN = '__TOKEN__'
  // An invisible window nobody can see must not take what a person types elsewhere: real
  // (trusted) keys and text input are dropped. The agent's own events are synthetic and pass.
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

  // Accessible name, in the order the spec and Localix's field labels use.
  const nameOf = (e, seen = new Set()) => {
    if (!e || seen.has(e)) return ''
    seen.add(e)
    const labelled = (e.getAttribute('aria-labelledby') || '').split(/\s+/).filter(Boolean)
      .map((id) => nameOf(document.getElementById(id), seen)).filter(Boolean).join(' ')
    const own = labelled || e.getAttribute('aria-label') ||
      [...(e.labels || [])].map((label) => label.innerText.trim()).filter(Boolean).join(' ') ||
      (['button', 'submit', 'reset'].includes(e.type) ? e.value : '') || e.getAttribute('alt') ||
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
  // is clicked directly, scrolled into view by the click, instead of costing a scroll decision.
  const justBelow = (r) =>
    r.width > 0 && r.height > 0 && r.right > 0 && r.left < innerWidth &&
    r.top >= innerHeight && r.top < innerHeight * 2

  const DIALOG = 'dialog[open],[role="dialog"],[role="alertdialog"],[aria-modal="true"]'

  function snapshot() {
    if (!document.body) return null
    for (const [id, e] of nodes) if (!e.isConnected) nodes.delete(id)
    const elements = []
    let omitted = 0
    for (const e of document.querySelectorAll(SELECTOR)) {
      if (secret(e) || !visible(e)) continue
      const role = roleOf(e)
      const box = e.getBoundingClientRect()
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
      if (e.closest(DIALOG)) element.dialog = true
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
      text: visibleText(),
      viewport: { w: innerWidth, h: innerHeight },
      scroll: { y: Math.round(scrollY), height: document.documentElement.scrollHeight },
      elements,
      omitted,
      busy: busy(),
      fields: fields(),
    }
  }

  // The form as it is filled in, including fields a modal hides from the element list (a date
  // picker covers "Where from?"): label and value, never a password's.
  function fields() {
    const out = []
    for (const e of document.querySelectorAll('input,textarea,select,[role="combobox"]')) {
      if (out.length >= 20 || secret(e) || e.type === 'password' || e.type === 'checkbox' || e.type === 'radio') continue
      if (!e.checkVisibility({ checkVisibilityCSS: true })) continue
      const value = valueOf(e, roleOf(e) || '').trim()
      const name = nameOf(e)
      if (value && name && !out.some((f) => f.name === name)) out.push({ name, value: value.slice(0, 80) })
    }
    return out
  }

  // Whether the page says it is still working: a document still loading, a busy region, a
  // progress bar, or words such as "Loading…" on screen.
  function busy() {
    if (document.readyState !== 'complete') return true
    const shown = (selector) => [...document.querySelectorAll(selector)]
      .some((e) => visible(e) && inViewport(e.getBoundingClientRect()))
    return shown('[aria-busy="true"],[role="progressbar"],progress') ||
      /\b(loading|please wait|searching)\b/i.test(visibleText().slice(0, 2000))
  }

  // The words a person can read in the viewport; off-screen bodies and footers stay out.
  function visibleText() {
    const words = []
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT)
    const range = document.createRange()
    let length = 0
    for (let node = walker.nextNode(); node && length < MAX_TEXT; node = walker.nextNode()) {
      const value = node.textContent.replace(/\s+/g, ' ').trim()
      const parent = node.parentElement
      if (!value || !parent || parent.closest('script,style,noscript,template') || !visible(parent)) continue
      range.selectNodeContents(node)
      if (!inViewport(range.getBoundingClientRect())) continue
      words.push(value)
      length += value.length + 1
    }
    return words.join('\n').slice(0, MAX_TEXT)
  }

