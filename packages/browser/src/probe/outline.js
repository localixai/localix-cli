  // The view as an outline, the way a person reads it: landmarks and headings, the words each
  // element says itself (a cell, a list item, a score), and every control with its ref, indented by
  // structure. What the element list cannot say: which question a radio answers, which result a
  // price belongs to. Only what is on screen or the screen below it, at most MAX_OUTLINE nodes.
  const MAX_OUTLINE = 400
  // A control is known by a short name; content (a paragraph, a code block of results) says more.
  const MAX_NAME = 200
  const MAX_TEXT_NODE = 1500
  const MAX_OUTLINE_CHARS = 12000
  const LANDMARK_TAGS = { NAV: 'navigation', MAIN: 'main', HEADER: 'banner', FOOTER: 'contentinfo',
    ASIDE: 'complementary', FORM: 'form', DIALOG: 'dialog', TABLE: 'table', UL: 'list', OL: 'list' }
  const LANDMARK_ROLES = /^(navigation|main|banner|contentinfo|complementary|search|region|form|dialog|alertdialog|table|list)$/
  // A control that holds the choices to act on: its options are walked, not folded into its name.
  const CHOICES = /^(listbox|menu|menubar|grid|treegrid|tree|tablist|radiogroup)$/
  const SKIP = /^(SCRIPT|STYLE|TEMPLATE|NOSCRIPT|HEAD|SVG|IFRAME|FRAME)$/i

  // An element's own words; separators alone ("|", "( )") say nothing.
  const ownText = (e) => {
    const text = [...e.childNodes]
      .filter((n) => n.nodeType === 3)
      .map((n) => n.textContent)
      .join(' ')
      .replace(/\s+/g, ' ')
      .trim()
    return /[\p{L}\p{N}]/u.test(text) ? text : ''
  }
  const quoted = (text, limit = MAX_NAME) => `"${text.slice(0, limit)}"`
  const contentRole = (e) => {
    const tag = e.tagName
    if (/^H[1-6]$/.test(tag)) return `heading ${tag[1]}`
    if (tag === 'LI') return 'listitem'
    if (tag === 'TD' || tag === 'TH') return 'cell'
    if (tag === 'LABEL' || tag === 'LEGEND') return 'label'
    return 'text'
  }

  function outline(doc = document, frame = null) {
    if (!doc.body) return ''
    const lines = []
    const near = (e) => {
      const r = shifted(e.getBoundingClientRect(), frame)
      return r.width > 0 && r.height > 0 && r.bottom > 0 && r.right > 0 && r.top < innerHeight * 2 && r.left < innerWidth
    }
    let chars = 0
    const emit = (depth, line) => {
      lines.push(`${'  '.repeat(Math.min(depth, 12))}- ${line}`)
      chars += line.length
      return lines.length < MAX_OUTLINE && chars < MAX_OUTLINE_CHARS
    }
    const walk = (e, depth) => {
      if (lines.length >= MAX_OUTLINE || chars >= MAX_OUTLINE_CHARS || SKIP.test(e.tagName) || !visible(e)) return
      const role = roleOf(e)
      if (role && e.matches(SELECTOR) && !secret(e)) {
        if (!near(e)) return
        const name = dayOf(e, nameOf(e) || contextOf(e))
        const value = valueOf(e, role)
        const states = statesOf(e)
        const label = [role, name && quoted(name), value && `= ${quoted(value)}`, states.length && `[${states.join(', ')}]`]
          .filter(Boolean).join(' ')
        if (!emit(depth, `${label} [ref=${refOf(e)}]`)) return
        if (!CHOICES.test(e.getAttribute('role') || '')) return
        for (const child of e.children) walk(child, depth + 1)
        return
      }
      let next = depth
      const explicit = e.getAttribute('role') || ''
      const landmark = LANDMARK_ROLES.test(explicit) ? explicit : LANDMARK_TAGS[e.tagName]
      if (/^H[1-6]$/.test(e.tagName) || explicit === 'heading') {
        const text = (e.innerText || '').replace(/\s+/g, ' ').trim()
        if (text && near(e)) emit(depth, `${contentRole(e)} ${quoted(text)}`)
        return
      }
      if (e.tagName === 'IMG') {
        const alt = (e.getAttribute('alt') || '').trim()
        if (alt && near(e)) emit(depth, `image ${quoted(alt)}`)
        return
      }
      const text = ownText(e)
      if (landmark && near(e)) {
        const name = e.getAttribute('aria-label') || ''
        emit(depth, name ? `${landmark} ${quoted(name)}` : landmark)
        next = depth + 1
      } else if (text && near(e)) {
        emit(depth, `${contentRole(e)} ${quoted(text, MAX_TEXT_NODE)}`)
        next = depth + 1
      }
      for (const child of e.shadowRoot ? e.shadowRoot.children : e.children) walk(child, next)
    }
    for (const child of doc.body.children) walk(child, 0)
    if (lines.length >= MAX_OUTLINE || chars >= MAX_OUTLINE_CHARS) lines.push('… (the outline stops here)')
    return lines.join('\n')
  }
