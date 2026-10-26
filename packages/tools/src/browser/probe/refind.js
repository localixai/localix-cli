  // What each control of the latest snapshot was: its role and name, and its place among the
  // controls of both. A page that renders its controls again (a framework's list) replaces a node
  // with its equal; the ref then finds the equal in the same place, while there are as many.
  // Kept for the latest refs (an earlier snapshot's still act), the oldest dropped past MARKS.
  const MARKS = 2000
  const marks = new Map()
  function mark(elements) {
    const counts = new Map()
    const now = elements.map(({ ref, role, name }) => {
      const key = `${role}\n${name}`
      const nth = counts.get(key) ?? 0
      counts.set(key, nth + 1)
      return [ref, { key, nth }]
    })
    for (const [ref, m] of now) {
      marks.delete(ref)
      marks.set(ref, { ...m, of: counts.get(m.key) })
    }
    for (const ref of marks.keys()) {
      if (marks.size <= MARKS) break
      marks.delete(ref)
    }
  }
  function again(ref) {
    const m = marks.get(ref)
    if (!m) return null
    const equals = []
    let n = 0
    for (const { e, role } of offered(subframes())) {
      if (n++ >= MAX_ELEMENTS) break
      if (`${role}\n${dayOf(e, labelOf(e))}` === m.key) equals.push(e)
    }
    const e = equals.length === m.of ? equals[m.nth] : null
    if (e) {
      ids.set(e, Number(ref.slice(1)))
      nodes.set(Number(ref.slice(1)), e)
    }
    return e
  }

