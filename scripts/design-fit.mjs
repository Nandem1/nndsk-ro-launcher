// Measure painted content, not just document scrollWidth. Content in the one
// designated scroll region is allowed offscreen, but must remain reachable.
export async function auditFit(page) {
  return page.evaluate(() => {
    const failures = []
    const panels = [
      ...document.querySelectorAll('section.rounded-panel, [role="dialog"]'),
    ]
    const rect = (element) => element.getBoundingClientRect()
    const intersect = (a, b) => ({
      left: Math.max(a.left, b.left),
      top: Math.max(a.top, b.top),
      right: Math.min(a.right, b.right),
      bottom: Math.min(a.bottom, b.bottom),
    })
    const viewport = { left: 0, top: 0, right: innerWidth, bottom: innerHeight }
    const positive = (r) => r.right > r.left + 1 && r.bottom > r.top + 1
    const contains = (a, b) =>
      b.left >= a.left - 1 &&
      b.right <= a.right + 1 &&
      b.top >= a.top - 1 &&
      b.bottom <= a.bottom + 1
    const isScroll = (element) =>
      !element.matches('input, textarea') &&
      /auto|scroll/.test(getComputedStyle(element).overflowY)
    const designated = (element) =>
      element.matches(
        '[data-design-panel-scroll], [data-design-rail-scroll], [role="listbox"]',
      )
    const name = (panel) =>
      panel.querySelector('h2')?.textContent ??
      panel.getAttribute('aria-label') ??
      'modal'
    const label = (element) =>
      element.getAttribute('aria-label') ??
      element.getAttribute('title') ??
      (element.value || element.textContent).trim().slice(0, 80)
    const visible = (element, bounds = rect(element)) => {
      let result = intersect(bounds, viewport)
      if (getComputedStyle(element).position === 'fixed') return result
      for (
        let parent = element.parentElement;
        parent;
        parent = parent.parentElement
      ) {
        const css = getComputedStyle(parent)
        if (
          /(auto|scroll|hidden|clip)/.test(`${css.overflowX} ${css.overflowY}`)
        )
          result = intersect(result, rect(parent))
        if (css.position === 'fixed') break
      }
      return result
    }
    const scrolls = [...document.querySelectorAll('*')].filter(isScroll)
    for (const scroll of scrolls) {
      if (!rect(scroll).height || !scroll.checkVisibility()) continue
      let parent = null
      for (
        let ancestor = scroll.parentElement;
        ancestor;
        ancestor = ancestor.parentElement
      ) {
        if (getComputedStyle(ancestor).position === 'fixed') break
        if (isScroll(ancestor)) {
          parent = ancestor
          break
        }
      }
      if (parent && isScroll(parent))
        failures.push({ kind: 'nested-scroll', text: label(scroll) })
      if (scroll.scrollHeight > scroll.clientHeight + 1 && !designated(scroll))
        failures.push({ kind: 'undesignated-scroll', text: label(scroll) })
      if (scroll.scrollWidth > scroll.clientWidth + 1)
        failures.push({ kind: 'horizontal-scroll', text: label(scroll) })
    }
    const results = []
    for (const panel of panels) {
      const boundary = visible(panel)
      if (!positive(boundary)) continue
      if (
        !contains(viewport, rect(panel)) &&
        !panel.closest('[data-design-rail-scroll]')
      )
        failures.push({ kind: 'panel-viewport', panel: name(panel) })
      const panelScrolls = [
        ...panel.querySelectorAll('[data-design-panel-scroll]'),
      ]
      if (panelScrolls.length > 1)
        failures.push({ kind: 'multiple-panel-scrolls', panel: name(panel) })
      for (const element of panel.querySelectorAll('*')) {
        if (
          !(element instanceof HTMLElement) ||
          !rect(element).height ||
          !element.checkVisibility() ||
          getComputedStyle(element).visibility === 'hidden'
        )
          continue
        const hasContent =
          [...element.childNodes].some(
            (node) =>
              node.nodeType === Node.TEXT_NODE && node.textContent.trim(),
          ) || element.matches('button,input,textarea,select')
        if (!hasContent) continue
        const painted = visible(element)
        if (!positive(painted)) continue
        const scroll = element.closest(
          '[data-design-panel-scroll], [data-design-rail-scroll]',
        )
        const clamp = element.closest('[data-design-clamp="true"]')
        // The approved two-line notice has an explicit expansion affordance.
        if (!contains(boundary, painted) && !clamp)
          failures.push({
            kind: 'panel-escape',
            panel: name(panel),
            text: label(element),
          })
        // Find hidden clipping at any ancestor, including a fixed-height body.
        for (
          let parent = element.parentElement;
          parent && panel.contains(parent);
          parent = parent.parentElement
        ) {
          const css = getComputedStyle(parent)
          if (!/(hidden|clip|auto|scroll)/.test(css.overflowY)) continue
          const bounds = rect(element)
          const scrollBoundary =
            scroll && (parent === scroll || parent.contains(scroll))
          const tooltip =
            getComputedStyle(element).textOverflow === 'ellipsis' &&
            element.closest('[title]')
          if (
            !contains(rect(parent), bounds) &&
            !scrollBoundary &&
            !clamp &&
            !tooltip
          )
            failures.push({
              kind: 'partial-content',
              panel: name(panel),
              text: label(element),
            })
        }
        // A value deliberately ellipsized must expose its complete value.
        if (
          getComputedStyle(element).textOverflow === 'ellipsis' &&
          element.scrollWidth > element.clientWidth + 1 &&
          !element.closest('[title]')
        )
          failures.push({
            kind: 'ellipsis-without-tooltip',
            panel: name(panel),
            text: label(element),
          })
      }
      results.push({
        panel: name(panel),
        height: rect(panel).height,
        scrolls: panelScrolls.map((scroll) => ({
          height: scroll.clientHeight,
          content: scroll.scrollHeight,
          active: scroll.scrollHeight > scroll.clientHeight + 1,
        })),
      })
    }
    for (let a = 0; a < panels.length; a++)
      for (let b = a + 1; b < panels.length; b++) {
        if (
          panels[a].contains(panels[b]) ||
          panels[b].contains(panels[a]) ||
          panels[a].matches('[role="dialog"]') ||
          panels[b].matches('[role="dialog"]')
        )
          continue
        if (positive(intersect(visible(panels[a]), visible(panels[b]))))
          failures.push({
            kind: 'panel-overlap',
            panels: [name(panels[a]), name(panels[b])],
          })
      }
    for (const list of document.querySelectorAll('[role="listbox"]')) {
      const trigger = [...document.querySelectorAll('[aria-controls]')].find(
        (element) => element.getAttribute('aria-controls') === list.id,
      )
      const bounds = rect(list),
        anchor = trigger && rect(trigger)
      const gap =
        anchor &&
        Math.min(
          Math.abs(bounds.top - anchor.bottom),
          Math.abs(anchor.top - bounds.bottom),
        )
      if (
        !anchor ||
        !contains(viewport, bounds) ||
        Math.abs(bounds.left - anchor.left) > 1 ||
        Math.abs(bounds.width - anchor.width) > 1 ||
        gap > 5
      )
        failures.push({
          kind: 'select-anchor',
          bounds: {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
          },
          gap,
        })
    }
    return { panels: results, failures }
  })
}
