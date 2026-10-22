  // Same-origin frames a person sees as part of the page: a consent banner or an editor that a
  // script wrote into an iframe without an address of its own (FastCMP, many TCF banners). Their
  // controls are offered like the page's own. Cross-origin frames stay out of reach, and so do
  // small ones (ad slots).
  const FRAME_MIN_AREA = 0.03
  // A fixed frame on top of the page covering this much of the viewport is a modal: its controls
  // are offered as a dialog, the way a `[role=dialog]` banner's are.
  const OVERLAY_MIN_AREA = 0.1

  function subframes() {
    const out = []
    for (const frame of document.querySelectorAll('iframe,frame')) {
      let doc = null
      try { doc = frame.contentDocument } catch { continue }
      if (!doc?.body || !visible(frame)) continue
      const r = frame.getBoundingClientRect()
      if (!(inViewport(r) || justBelow(r)) || r.width * r.height < FRAME_MIN_AREA * innerWidth * innerHeight) continue
      out.push({ frame, doc, x: r.left + frame.clientLeft, y: r.top + frame.clientTop, overlay: overlay(frame, r) })
    }
    return out
  }

  function overlay(frame, r) {
    const left = Math.max(0, r.left), right = Math.min(innerWidth, r.right)
    const top = Math.max(0, r.top), bottom = Math.min(innerHeight, r.bottom)
    if (right <= left || bottom <= top || (right - left) * (bottom - top) < OVERLAY_MIN_AREA * innerWidth * innerHeight) return false
    if (topAt(document, (left + right) / 2, (top + bottom) / 2) !== frame) return false
    for (let n = frame; n; n = n.parentElement) if (getComputedStyle(n).position === 'fixed') return true
    return false
  }

  // A box inside a frame, in the top window's coordinates.
  const shifted = (r, f) => f
    ? { left: r.left + f.x, right: r.right + f.x, top: r.top + f.y, bottom: r.bottom + f.y, width: r.width, height: r.height }
    : r

  // The frame an element lives in, as `subframes` lists it, or null for the top document.
  function frameOf(e) {
    const owner = e.ownerDocument?.defaultView?.frameElement
    if (!owner) return null
    const r = owner.getBoundingClientRect()
    return { frame: owner, x: r.left + owner.clientLeft, y: r.top + owner.clientTop }
  }
