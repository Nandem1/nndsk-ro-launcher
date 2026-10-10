import fs from 'node:fs'
import path from 'node:path'

const properties = [
  'backgroundColor',
  'borderColor',
  'borderWidth',
  'borderRadius',
  'fontFamily',
  'fontSize',
  'fontWeight',
  'height',
  'paddingTop',
  'paddingRight',
  'paddingBottom',
  'paddingLeft',
]

// Compare painted geometry: 50% and 9999px both resolve to the same circle.
async function read(
  locator,
  { panel = false, row = false, notice = false } = {},
) {
  return locator.evaluate(
    (element, options) => {
      const css = getComputedStyle(element)
      const rect = element.getBoundingClientRect()
      const radius = (value) =>
        Math.min(
          value.endsWith('%')
            ? (rect.height * parseFloat(value)) / 100
            : parseFloat(value),
          rect.width / 2,
          rect.height / 2,
        )
      const font =
        options.row || options.notice
          ? getComputedStyle(
              element.querySelector(
                options.notice
                  ? '.tx, p'
                  : '.text-ink, span[style*="font-weight"]',
              ) ?? element,
            )
          : css
      const result = Object.fromEntries(
        [
          'backgroundColor',
          'borderColor',
          'borderWidth',
          'fontFamily',
          'fontSize',
          'fontWeight',
          'paddingTop',
          'paddingRight',
          'paddingBottom',
          'paddingLeft',
        ].map((key) => [key, (key.startsWith('font') ? font : css)[key]]),
      )
      result.height = `${rect.height}px`
      result.borderRadius = `${radius(css.borderTopLeftRadius)}px`
      if (options.panel) {
        // Existing DOM splits panel insets between header and body. Measure the
        // effective outer insets, not an invented change to the panel structure.
        const header = getComputedStyle(element.firstElementChild)
        const body = getComputedStyle(element.lastElementChild)
        Object.assign(result, {
          paddingTop: header.paddingTop,
          paddingRight: header.paddingRight,
          paddingBottom: body.paddingBottom,
          paddingLeft: body.paddingLeft,
        })
      }
      return result
    },
    { panel, row, notice },
  )
}

// Chromium doesn't expose native range subparts through getComputedStyle.
// Resolve the actual emitted pseudo rules in temporary probes; no expected
// colors/sizes are supplied to those probes. Root variables stay inherited.
async function rangePart(page, input, pseudo) {
  return input.evaluate((element, pseudo) => {
    const probe = document.createElement('div')
    const rules = []
    const collect = (sheet) => {
      for (const rule of sheet.cssRules) {
        if (rule.cssRules?.length) collect(rule)
        if (
          rule.selectorText?.endsWith(pseudo) &&
          element.matches(rule.selectorText.slice(0, -pseudo.length))
        )
          rules.push(rule)
      }
    }
    for (const sheet of document.styleSheets) collect(sheet)
    if (!rules.length) throw new Error(`No emitted range rule for ${pseudo}`)
    // Preserve variable-bearing shorthands: CSSOM longhands of an unresolved
    // shorthand can be empty strings (pending substitution).
    for (const rule of rules) probe.style.cssText += rule.style.cssText
    probe.style.setProperty(
      '--range-progress',
      element.style.getPropertyValue('--range-progress'),
    )
    probe.style.position = 'fixed'
    probe.style.visibility = 'hidden'
    probe.style.width ||= '100px'
    element.parentElement.append(probe)
    const css = getComputedStyle(probe)
    const result = Object.fromEntries(
      [
        'backgroundColor',
        'borderColor',
        'borderWidth',
        'fontFamily',
        'fontSize',
        'fontWeight',
        'height',
        'paddingTop',
        'paddingRight',
        'paddingBottom',
        'paddingLeft',
        'backgroundImage',
        'backgroundSize',
      ].map((key) => [key, css[key]]),
    )
    result.borderRadius = `${Math.min(parseFloat(css.borderRadius), parseFloat(css.height) / 2)}px`
    probe.remove()
    return result
  }, pseudo)
}

const equivalent = (key, a, b) => {
  if (key.endsWith('Color')) {
    const color = (value) => (value.match(/[\d.]+/g) ?? []).map(Number)
    const aa = color(a),
      bb = color(b)
    return (
      aa.length === bb.length &&
      aa.every((v, i) => Math.abs(v - bb[i]) <= (i === 3 ? 0.001 : 1))
    )
  }
  if (key === 'fontFamily')
    return (
      a.replace(/['"]/g, '').toLowerCase() ===
      b.replace(/['"]/g, '').toLowerCase()
    )
  if (key === 'fontWeight') return a === b
  return Math.abs(parseFloat(a) - parseFloat(b)) <= 1
}

export function conformanceFailures(rows) {
  return rows.flatMap((row) =>
    row.differences
      .filter((difference) => !difference.reason)
      .map((difference) => ({ component: row.component, ...difference })),
  )
}

export async function reviewConformance({ reference, page, port, output }) {
  const board = reference.locator('body > div').first()
  await page.goto(`http://127.0.0.1:${port}/?fixture=pending`)
  await page.getByText('DXVK · pendiente', { exact: true }).waitFor()
  await page.evaluate(() => document.fonts.ready)
  await page.mouse.move(1439, 899)
  const panel = (name) =>
    page
      .getByRole('heading', { name, exact: true })
      .locator('xpath=ancestor::section')
  const server = panel('Servidor')
  const spammer = panel('Spammer')
  const pot = panel('AutoPot')
  const runner = panel('Runner predeterminado')
  const refPanels = board.locator('.cd')
  const rows = []
  if (process.env.RO_DESIGN_CONFORMANCE_MUTATION === '1')
    await server.evaluate((element) => {
      element.style.borderColor = 'rgb(255,0,0)'
      element.getAnimations().forEach((animation) => animation.finish())
    })
  const add = (component, ref, app, reasons = {}, { text = true } = {}) => {
    const differences = properties
      .filter((key) => !equivalent(key, ref[key], app[key]))
      .map((property) => ({
        property,
        reference: ref[property],
        app: app[property],
        reason:
          reasons[property] ??
          (!text && property.startsWith('font')
            ? 'Sin texto pintado; la tipografía se mide en su etiqueta o valor.'
            : property === 'borderColor' &&
                parseFloat(ref.borderWidth) === 0 &&
                parseFloat(app.borderWidth) === 0
              ? 'Borde no dibujado (0px); currentColor heredado no es un borde visible.'
              : undefined),
      }))
    rows.push({ component, reference: ref, app, differences })
  }
  const pair = async (component, ref, app, reasons, options) =>
    add(
      component,
      await read(ref, { row: options?.row, notice: options?.notice }),
      await read(app, options),
      reasons,
      options,
    )
  await pair(
    'Panel',
    refPanels.first(),
    server,
    {
      height:
        'Contenido real: cinco nombres largos, separación y acciones existentes; panel fluido, sin cambiar layout.',
    },
    { panel: true, text: false },
  )
  await pair(
    'Botón secundario',
    board.getByText('Importar y usar nndsk-ro-proton', { exact: true }),
    page.getByRole('button', {
      name: 'Importar y usar nndsk-ro-proton',
      exact: true,
    }),
  )
  await pair(
    'Botón primario grande',
    board.getByText('Preparar entorno', { exact: true }),
    page.getByRole('button', { name: 'Preparar entorno', exact: true }),
  )
  // The reference defines .bt.g but doesn't instantiate one. Measure that
  // existing rule in its own font/context rather than inventing a reference.
  await board
    .locator('.cd')
    .first()
    .evaluate((element) => {
      const probe = document.createElement('span')
      probe.className = 'bt g'
      probe.dataset.conformance = 'ghost'
      probe.textContent = 'Ghost'
      element.append(probe)
    })
  await pair(
    'Botón ghost',
    reference.locator('[data-conformance="ghost"]'),
    page.getByRole('button', { name: 'Agregar servidor', exact: true }),
    {
      height: 'El ghost de producción es un IconButton funcional compacto.',
      paddingTop:
        'IconButton sin padding: conserva su tamaño accesible existente.',
      paddingRight:
        'IconButton sin padding: conserva su tamaño accesible existente.',
      paddingBottom:
        'IconButton sin padding: conserva su tamaño accesible existente.',
      paddingLeft:
        'IconButton sin padding: conserva su tamaño accesible existente.',
    },
    { text: false },
  )
  await reference
    .locator('[data-conformance="ghost"]')
    .evaluate((e) => e.remove())
  await pair(
    'Campo/select',
    refPanels.nth(1).locator('.in'),
    runner.locator('[aria-haspopup="listbox"]'),
  )
  const off = pot.getByRole('switch').nth(1)
  const onRef = reference.locator('body > div').nth(1).locator('.tg.on').first()
  await pair(
    'Switch apagado: pista',
    board.locator('.tg').first(),
    off,
    {},
    { text: false },
  )
  await pair(
    'Switch apagado: perilla',
    board.locator('.tg b').first(),
    off.locator('span'),
    {},
    { text: false },
  )
  await off.click()
  await off.evaluate((e) =>
    Promise.all(e.getAnimations().map((a) => a.finished)),
  )
  await pair('Switch encendido: pista', onRef, off, {}, { text: false })
  await pair(
    'Switch encendido: perilla',
    onRef.locator('b'),
    off.locator('span'),
    {},
    { text: false },
  )
  await off.click()
  await off.evaluate((e) =>
    Promise.all(e.getAnimations().map((a) => a.finished)),
  )
  for (const [state, selected] of [
    ['sin selección', false],
    ['seleccionado', true],
  ]) {
    const input = server
      .locator(`input[type="radio"]:${selected ? 'checked' : 'not(:checked)'}`)
      .first()
    const refRadio = refPanels
      .first()
      .locator(selected ? '.rd.on' : '.rd:not(.on)')
      .first()
    await pair(
      `Radio de servidor ${state}`,
      refRadio,
      input,
      selected
        ? {}
        : {
            borderColor:
              'Decisión aprobada: outline #6B6F76 sustituye #4A4E55 para ≥3:1.',
          },
      { text: false },
    )
    await pair(
      `Fila de servidor ${state}`,
      refPanels
        .first()
        .locator(selected ? '.rw.on' : '.rw:not(.on)')
        .first(),
      input.locator('xpath=../..'),
      {},
      { row: true },
    )
    await pair(
      `Tecla ${state}`,
      board.locator(selected ? '.ky.on' : '.ky:not(.on)').first(),
      spammer
        .getByRole('button', { name: selected ? 'F1' : 'F2', exact: true })
        .first(),
    )
  }
  await pair(
    'Contenedor segmentado',
    board.locator('.sg'),
    page.getByRole('button', { name: 'Combate', exact: true }).locator('..'),
    {},
    { text: false },
  )
  await pair(
    'Segmento activo',
    board.locator('.sg .on'),
    page.getByRole('button', { name: 'Combate', exact: true }),
  )
  const range = pot.locator('input[type="range"]').first()
  const track = await rangePart(page, range, '::-webkit-slider-runnable-track')
  add(
    'Deslizador: pista',
    await read(board.locator('.sl').first()),
    track,
    {},
    { text: false },
  )
  const fill = {
    ...track,
    backgroundColor: track.backgroundImage.match(/rgb\([^)]*\)/)?.[0],
  }
  if (!fill.backgroundColor || !/^[\d.]+% 100%$/.test(track.backgroundSize))
    throw new Error('Missing range fill')
  add(
    'Deslizador: relleno',
    await read(board.locator('.sl i').first()),
    fill,
    {},
    { text: false },
  )
  add(
    'Deslizador: pulgar',
    await read(board.locator('.sl u').first()),
    await rangePart(page, range, '::-webkit-slider-thumb'),
    {},
    { text: false },
  )
  await pair(
    'Barra HP/SP',
    board.locator('.br').first(),
    pot.locator('.h-1\\.5').first(),
    {},
    { text: false },
  )
  await pair(
    'Bloque de aviso',
    board.locator('.nt').first(),
    panel('Herramientas').locator('.notice-warn').first(),
    {
      height:
        'Avisos completos y hash/FileVersion reales; el bloque crece sin cortar texto.',
      paddingLeft:
        'Punto con ::before, no un hijo nuevo: 14px exteriores + 8px punto + 10px separación = 32px hasta el texto.',
    },
    { notice: true },
  )
  await pair(
    'Punto de estado',
    refPanels.nth(3).locator('.dt').first(),
    panel('Herramientas').locator('.rounded-pill.bg-ok').first(),
    {},
    { text: false },
  )
  await pair(
    'Microetiqueta',
    board.getByText('Teclas', { exact: true }),
    spammer.getByText('Teclas', { exact: true }),
  )
  await pair(
    'Título de panel',
    refPanels.first().locator('.tt'),
    server.getByRole('heading', { name: 'Servidor', exact: true }),
  )
  await pair(
    'Wordmark',
    board.getByText('RO-Launcher', { exact: true }),
    page.getByRole('heading', { name: 'RO-Launcher', exact: true }),
  )
  const failures = conformanceFailures(rows)
  const format = (value) =>
    `bg ${value.backgroundColor}; borde ${value.borderWidth}/${value.borderColor}; r ${value.borderRadius}; ${value.fontFamily} ${value.fontSize}/${value.fontWeight}; h ${value.height}; p ${[value.paddingTop, value.paddingRight, value.paddingBottom, value.paddingLeft].join(' ')}`
  const table = rows
    .map(
      (row) =>
        `| ${row.component} | ${format(row.reference)} | ${format(row.app)} | ${row.differences.length ? row.differences.map((d) => `${d.property}: ${d.reason ?? 'DESVIACIÓN SIN JUSTIFICAR'}`).join('<br>') : 'Conforme'} |`,
    )
    .join('\n')
  fs.writeFileSync(
    path.join(output, 'conformance.md'),
    `# Conformidad computada · grafito suave\n\nReferencia inmutable refined.html y build de producción, Chromium, 1440×900. Colores ±1 canal; tamaños/radios ±1px; familia/peso exactos. Radios circulares normalizados a su radio pintado. Insets de Panel medidos en cabecera/cuerpo existentes. Pseudo-elementos nativos del range medidos mediante las reglas emitidas resueltas en probes temporales (no se suministran valores esperados).\n\nDecisiones autorizadas: outline #6B6F76; foco sólido 2px solo al navegar por teclado; line-soft, line y track apagado decorativos. No se modifica el archivo de referencia.\n\n| Componente | Referencia (computado) | App (computado) | Estado / justificación |\n| --- | --- | --- | --- |\n${table}\n\nDesviaciones sin justificar: ${failures.length}. El arnés devuelve 1 si este número no es cero. Las diferencias de altura justificadas son exclusivamente contenido real o el control funcional con icono, no cambios de layout.\n`,
  )
  return { rows, failures }
}
