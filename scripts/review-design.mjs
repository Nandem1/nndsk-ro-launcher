import fs from 'node:fs'
import path from 'node:path'
import { chromium } from 'playwright'
import { reviewConformance } from './design-conformance.mjs'
import { auditFit } from './design-fit.mjs'

const minimum = process.argv.includes('--minimum')
const output = path.resolve(
  process.env.RO_DESIGN_REVIEW_OUTPUT ??
    (minimum ? '/tmp/ro-soft-fit-minimum' : 'docs/design-review'),
)
const baseline = process.env.RO_DESIGN_BASELINE === '1'
const port =
  process.argv.slice(2).find((argument) => !argument.startsWith('--')) ?? '5175'
const width = minimum ? 1280 : 1440
const height = minimum ? 820 : 900
const executablePath = process.env.RO_DESIGN_CHROMIUM ?? '/usr/bin/chromium'
fs.mkdirSync(output, { recursive: true })
const csp = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf8')).app
  .security.csp
const checks = []
const screenshotScenes = new Set([
  'prep-empty',
  'prep-combat-fit',
  'prep-buffs',
  'active-editors',
  'ingame-buffs',
  'server',
  'scanner',
  'prep-ready',
])
const switchContrast = []
const stateContrast = []
let selectCheck = null
let disclosureCheck = null
let railCheck = null
let controlCheck = null
const diagnosticChecks = []
const contrastFailures = []
let conformance = null
const fitDetails = { notices: [], buffs: [], readyActions: [] }
const browser = await chromium.launch({ executablePath, headless: true })
const page = await browser.newPage({ viewport: { width, height } })
await page.route('**/favicon.ico', (route) =>
  route.fulfill({
    status: 200,
    contentType: 'image/png',
    body: Buffer.from(
      'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLbtAAAAABJRU5ErkJggg==',
      'base64',
    ),
  }),
)
await page.route(`http://127.0.0.1:${port}/?*`, async (route) => {
  const response = await route.fetch()
  await route.fulfill({
    response,
    headers: { ...response.headers(), 'content-security-policy': csp },
  })
})
const errors = []
const fontResponses = []
page.on('response', (response) => {
  if (/\.woff2?(?:$|\?)/.test(response.url()))
    fontResponses.push({
      url: new URL(response.url()).pathname,
      status: response.status(),
    })
})
page.on('pageerror', (error) => errors.push(error.message))
page.on('console', (message) => {
  if (message.type() === 'error') errors.push(message.text())
})
await page.addInitScript(() => {
  const fit = location.search.includes('fixture=fit-')
  const fitRules =
    location.search.includes('fit-ingame') ||
    location.search.includes('fit-starting')
      ? 8
      : 6
  const pending = location.search.includes('fixture=pending')
  const realistic =
    location.search.includes('fixture=realistic') || pending || fit
  const server = {
    id: 'design-fixture',
    name: 'TestRO',
    executablePath: '/fixture/ragexe.exe',
    prefixMode: 'isolated',
    spammer: {
      keys: ['F1'],
      shiftMode: { enabled: true, triggerKeys: ['F1'] },
      gearSwitch: {
        enabled: true,
        switchDelayMs: 50,
        rules: [{ trigger: 'F1', atkKeys: ['F2'], defKeys: ['F3'] }],
      },
    },
  }
  const runner = {
    id: 'nndsk-ro-proton-0.1.0-dev.2',
    name: pending ? 'nndsk-ro-proton 0.1.0-dev.2' : 'nndsk-ro-proton',
    path: '/fixture/proton',
  }
  if (fit) {
    server.name = 'SakuraRO'
    server.runner =
      '/home/nndsk/.local/share/ro-launcher/runners/wine-7.16-staging-tkg-amd64/bin/wine'
    server.autobuff = {
      enabled: false,
      delayMs: 100,
      rules: [
        'Concentration Potion',
        'Awakening Potion',
        'Berserk Potion',
        'Fireproof Potion',
        'Waterproof Potion',
        'Windproof Potion',
        'Earthproof Potion',
        'Box of Gloom',
      ]
        .slice(0, fitRules)
        .map((label, index) => ({
          id: `rule-${index}`,
          label,
          statusId: [37, 38, 39, 910, 908, 911, 909, 3][index],
          key: `F${index + 1}`,
          cooldownMs: 1000 + index * 100,
          priority: (index + 1) * 10,
          enabled: true,
        })),
    }
  }
  const tool = { found: false, path: null, label: null }
  const deps = {
    wine: true,
    winetricks: true,
    dxvk: true,
    prefixConfigured: true,
    audioOk: true,
    audioDriver: 'pulse',
    audioStack: 'pipewire',
    audioWarning: null,
    inputGroupOk: true,
    inputGroupWarning: null,
    uinputInputOk: true,
    uinputInputWarning: null,
    prefixOk: true,
    prefixWarning: null,
    dxvkOk: true,
    dxvkWarning: null,
    runnerKind: 'proton',
    runnerOk: true,
    runnerWarning: null,
    prefixPath: realistic
      ? '/fixture/prefixes/honeyro/nndsk-ro-proton-verified-isolated-prefix'
      : '/fixture/prefix',
    prefixScope: 'isolated',
    prefixManaged: true,
    readyToLaunch: true,
    canSetup: true,
    canReset: true,
    checks: [],
  }
  if (realistic) {
    deps.audioWarning =
      'PipeWire activo; se verificó la salida de audio para el runtime administrado.'
    deps.inputGroupWarning =
      'Acceso de entrada verificado para la sesión actual; dispositivos aislados por cliente.'
    deps.prefixWarning =
      'Prefix administrado conservado; arquitectura x86 y componentes instalados verificados.'
  }
  if (pending) {
    Object.assign(deps, {
      wine: false,
      winetricks: false,
      dxvk: false,
      prefixConfigured: false,
      runnerOk: false,
      runnerWarning:
        'Runtime administrado pendiente. Al preparar el entorno se comprobarán UMU y Proton; solo se descargará lo que falte o no pase la validación',
      prefixOk: false,
      prefixWarning:
        'El entorno se validará con el runtime administrado antes de jugar',
      dxvkOk: false,
      dxvkWarning: 'DXVK está incluido en el runtime administrado',
      audioWarning: null,
      inputGroupWarning: null,
      readyToLaunch: false,
      canReset: false,
      checks: ['runner', 'prefix', 'dxvk'].map((id) => ({
        id,
        severity:
          location.search.includes('fault=runner') && id === 'runner'
            ? 'error'
            : 'pending',
        message: `${id} pendiente`,
        remediation: null,
      })),
      compatibility: {
        assessment: { kind: 'unknown' },
        recommendation: {
          profile: 'managedNndskRoProton',
          evidenceId: 'gepard-26.8.26.1-nndsk-ro-proton-0.1.0-dev.2',
          reason: 'Compatibilidad por verificar',
        },
      },
    })
  }
  if (fit && !/fit-ready|fit-starting|fit-ingame/.test(location.search)) {
    deps.readyToLaunch = false
    deps.prefixOk = false
    deps.prefixWarning =
      'El entorno se validará con el runtime administrado antes de jugar'
    deps.canReset = false
  }
  const tools = {
    gameDir: '/fixture',
    openSetup: tool,
    patcher: tool,
    dgvoodoo: {
      cpl: tool,
      d3dimmDll: tool,
      ddrawDll: tool,
      conf: tool,
      configured: false,
      needsInstall: false,
      canAutoInstall: false,
      canUninstall: false,
      issues: [],
    },
    diagnostics: {
      architecture: 'x86',
      graphicsApis: ['d3d9'],
      managedPatcher: false,
      webview2Required: false,
      peAnalysisConclusive: true,
      gepardPresent: false,
      gameguardPresent: false,
      warnings: [],
    },
  }
  if (realistic) {
    tools.openSetup = {
      found: true,
      path: '/fixture/Setup.exe',
      label: 'Setup.exe',
    }
    tools.patcher = {
      found: true,
      path: '/fixture/HoneyRO Patcher.exe',
      label: 'HoneyRO Patcher.exe',
    }
    tools.dgvoodoo.cpl = {
      found: true,
      path: '/fixture/dgVoodooCpl.exe',
      label: 'dgVoodooCpl.exe',
    }
    tools.dgvoodoo.configured = true
    tools.diagnostics.gepardPresent = true
    tools.diagnostics.warnings = [
      'Se detectó anti-cheat. Confirma con el servidor si Wine, DXVK y la versión de dgVoodoo están permitidos.',
      'Gepard Shield 3.0 (FileVersion 26.8.26.1, SHA-256 e2f624d2e345...) reconocido; recomendación nndsk-ro-proton administrado. Validated se confirma en Dependencias si el runtime coincide.',
    ]
  }
  if (fit) {
    tools.diagnostics.graphicsApis = ['DirectDraw', 'Direct3D 9']
    tools.diagnostics.warnings = [
      'El cliente carga DirectDraw y Direct3D 9; cambiar el modo gráfico no evita resolver ambas cadenas de DLL.',
      'Una dependencia local usa WebView2Loader; el loader no sustituye al Edge WebView2 Runtime.',
      'El patcher es administrado (.NET); Wine Mono puede renderizar distinto a .NET Framework nativo.',
      'Se detectó anti-cheat. Confirma con el servidor si Wine, DXVK y la versión de dgVoodoo están permitidos.',
      'Build de Gepard no validada (SHA-256 e89eb47d8386...); conserva un prefix separado al probar runners.',
    ]
  }
  const responses = {
    list_servers: location.search.includes('fixture=empty')
      ? []
      : realistic
        ? [
            server,
            ...[
              'HoneyRO — entorno de prueba extendido',
              'SakuraRO — compatibilidad verificada',
              'Servidor comunitario — revisión pendiente',
              'Ragnarok Online — configuración alternativa',
            ].map((name, index) => ({
              ...server,
              id: `design-fixture-${index + 2}`,
              name,
            })),
          ]
        : [server],
    load_settings: { defaultRunner: runner.path, richPresenceEnabled: false },
    list_runners: [runner],
    list_game_clients: [],
    list_client_profiles: [],
    take_storage_notices: [],
    list_runtime_observations: [],
    list_runtime_benchmarks: [],
    check_dependencies: deps,
    scan_server_tools: tools,
    get_update_snapshot: {
      currentVersion: '0.2.1',
      phase: { kind: 'unavailable', reason: 'notPackaged' },
    },
  }
  let callbackId = 0
  const callbacks = new Map()
  const listeners = new Map()
  window.__designEmit = (event, payload) => {
    for (const [id, listener] of listeners) {
      if (listener.event === event)
        callbacks.get(listener.handler)?.({ event, id, payload })
    }
  }
  window.__TAURI_INTERNALS__ = {
    transformCallback: (callback) => {
      const id = ++callbackId
      callbacks.set(id, callback)
      return id
    },
    unregisterCallback: () => {},
    metadata: {
      currentWindow: { label: 'main' },
      currentWebview: { label: 'main' },
    },
    invoke: async (command, args) => {
      if (command === 'launch_game' && location.search.includes('fit-starting'))
        return new Promise(() => {})
      if (command === 'plugin:app|version') return '0.2.1'
      if (command === 'plugin:event|listen') {
        const id = ++callbackId
        listeners.set(id, args)
        return id
      }
      if (command === 'plugin:event|unlisten') {
        listeners.delete(args.eventId)
        return null
      }
      if (command in responses) return structuredClone(responses[command])
      if (/update/.test(command)) return responses.get_update_snapshot
      return null
    },
  }
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }
})

const capture = async (name) => {
  await page.evaluate(() => document.fonts.ready)
  checks.push({
    scene: name,
    ...(await auditLayout()),
    ...(await auditBoxes()),
    ...(await auditLines()),
    fit: await auditFit(page),
  })
  await page.evaluate(() => {
    for (const animation of document.getAnimations()) {
      if (animation.effect?.getTiming().iterations === Infinity) {
        animation.pause()
        animation.currentTime = 0
      } else animation.finish()
    }
  })
  await page.evaluate(
    () =>
      new Promise((done) =>
        requestAnimationFrame(() => requestAnimationFrame(done)),
      ),
  )
  if (!baseline)
    stateContrast.push({ scene: name, ...(await auditStateContrast()) })
  checks.at(-1).space = await page.evaluate(() => {
    const rect = (selector) => {
      const bounds = document.querySelector(selector)?.getBoundingClientRect()
      return bounds ? { width: bounds.width, height: bounds.height } : null
    }
    return {
      rail: rect('[data-design-rail-scroll]'),
      tools: rect('[data-design-tool-body]'),
      logs: rect('[data-design-logs]'),
    }
  })
  const styles = await page.evaluate(() =>
    [...document.querySelectorAll('body, body *')]
      .filter((el) => !['SCRIPT', 'STYLE'].includes(el.tagName))
      .map((el) => {
        const css = getComputedStyle(el)
        const rect = el.getBoundingClientRect()
        return {
          tag: el.tagName,
          text: el.childElementCount === 0 ? el.textContent : '',
          role: el.getAttribute('role'),
          accessibleLabel: el.getAttribute('aria-label'),
          classes: el.getAttribute('class'),
          rect: [rect.x, rect.y, rect.width, rect.height],
          styles: Object.fromEntries(
            [
              'color',
              'background-color',
              'border-color',
              'border-radius',
              'box-shadow',
              'font-size',
              'line-height',
              'font-family',
              'background-image',
              'backdrop-filter',
              'padding',
              'gap',
              'opacity',
              'filter',
              'transform',
              'visibility',
              'accent-color',
              'animation-name',
              'animation-duration',
              'animation-play-state',
              'animation-timing-function',
              'animation-fill-mode',
              'transition-duration',
              'border-width',
            ].map((key) => [key, css.getPropertyValue(key)]),
          ),
        }
      }),
  )
  // Keep detailed diagnostics outside the repository.
  fs.writeFileSync(
    `/tmp/ro-soft-${baseline ? 'before' : 'after'}-${width}x${height}-${name}.json`,
    JSON.stringify(styles, null, 2),
  )
  if (baseline || screenshotScenes.has(name))
    await page.screenshot({
      path: path.join(output, `${name}-${width}x${height}.png`),
    })
  process.stdout.write(`${name}: ${styles.length} elements rendered\n`)
}

async function auditLines() {
  return page.evaluate(() => {
    const panels = [...document.querySelectorAll('section.rounded-panel')]
    const lines = panels.map((panel) => {
      const elements = [...panel.querySelectorAll('*')].filter((element) => {
        if (!element.getBoundingClientRect().height) return false
        const classes = [...element.classList]
        const css = getComputedStyle(element)
        return ['t', 'b'].some(
          (side) =>
            classes.some((name) =>
              new RegExp(`^(?:[\\w-]+:)*border-${side}(?:-(?:0|2|4|8))?$`).test(
                name,
              ),
            ) &&
            parseFloat(
              css[side === 't' ? 'borderTopWidth' : 'borderBottomWidth'],
            ) > 0,
        )
      })
      return {
        panel: panel.querySelector('h2')?.textContent ?? 'rail',
        count: elements.length,
      }
    })
    const decorativeIcons = [
      ...panels.flatMap((panel) => [
        ...(panel.querySelector('h2')?.parentElement.querySelectorAll('svg') ??
          []),
      ]),
      ...[...document.querySelectorAll('button')]
        .filter((button) =>
          [
            'Combate',
            'Buffs',
            'Sharp Shooting / Focused Arrow Strike',
            'ATK / DEF Gear Switch',
          ].some((text) => button.textContent.trim().startsWith(text)),
        )
        .flatMap((button) => [
          ...button.querySelectorAll('svg:not(.lucide-chevron-down)'),
        ]),
    ]
    return {
      lines: {
        total: lines.reduce((sum, row) => sum + row.count, 0),
        panels: lines,
      },
      decorativeIcons: decorativeIcons.length,
    }
  })
}

async function verifyQuietControls() {
  const initialRings = await page.evaluate(() =>
    [...document.querySelectorAll('button, input, textarea, summary')]
      .filter((element) => {
        const css = getComputedStyle(element)
        return (
          (css.outlineStyle !== 'none' &&
            parseFloat(css.outlineWidth) > 0 &&
            !css.outlineColor.endsWith(', 0)')) ||
          css.boxShadow !== 'none'
        )
      })
      .map((element) => element.outerHTML.slice(0, 180)),
  )
  if (initialRings.length)
    throw new Error(`Rings on initial load: ${JSON.stringify(initialRings)}`)
  const field = page.getByRole('spinbutton', { name: 'Porcentaje de HP' })
  const key = page.getByRole('button', { name: 'F2', exact: true }).first()
  const secondary = page.getByRole('button', {
    name: 'Importar y usar nndsk-ro-proton',
    exact: true,
  })
  const select = page.locator('button[aria-haspopup="listbox"]').first()
  const readings = []
  for (const [name, locator] of [
    ['input', field],
    ['key', key],
    ['secondary', secondary],
    ['select', select],
  ]) {
    await page.evaluate(() => document.activeElement?.blur())
    await page.mouse.move(width - 1, height - 1)
    const read = () =>
      locator.evaluate((element) => {
        const css = getComputedStyle(element)
        const root = getComputedStyle(document.documentElement)
        const rgb = (name) =>
          `rgb(${root.getPropertyValue(`--c-${name}`).trim().split(/\s+/).join(', ')})`
        return {
          border: css.borderTopColor,
          outline: css.outlineColor,
          outlineWidth: css.outlineWidth,
          outlineStyle: css.outlineStyle,
          shadow: css.boxShadow,
          background: css.backgroundColor,
          borderWidth: css.borderTopWidth,
          line: rgb('line'),
          strong: rgb('line-strong'),
          raised: rgb('panel-raised'),
          accent: rgb('accent'),
        }
      })
    const resting = await read()
    await locator.hover()
    await locator.evaluate((element) =>
      Promise.all(
        element.getAnimations().map((animation) => animation.finished),
      ),
    )
    const hover = await read()
    await locator.click()
    const mouseFocus = await read()
    if (mouseFocus.outlineStyle !== 'none' || mouseFocus.shadow !== 'none')
      throw new Error(
        `Pointer focus ring: ${JSON.stringify({ name, mouseFocus })}`,
      )
    if (name === 'key') await locator.click() // restore the fixture's selection
    if (name === 'select') await page.mouse.click(width - 1, height - 1)
    await locator.focus()
    await page.keyboard.press('Tab')
    await page.keyboard.press('Shift+Tab')
    const focus = await read()
    if (
      (['input', 'select'].includes(name)
        ? resting.border !== resting.line || hover.border !== hover.strong
        : resting.background !== resting.raised ||
          hover.background !== resting.line ||
          resting.borderWidth !== '0px') ||
      focus.outlineWidth !== '2px' ||
      focus.outline !== focus.accent ||
      focus.shadow !== 'none'
    )
      throw new Error(
        `Quiet control failed: ${JSON.stringify({ name, resting, hover, focus })}`,
      )
    readings.push({ name, resting, hover, mouseFocus, focus })
  }
  await page.mouse.move(width - 1, height - 1)
  await page.evaluate(() => document.activeElement?.blur())
  return readings
}

async function verifyServerRowInteraction() {
  const radio = page.locator('input[type="radio"]').nth(1)
  const row = radio.locator('xpath=../..')
  await page.evaluate(() => document.activeElement?.blur())
  await page.mouse.move(width - 1, height - 1)
  const rowRead = () =>
    row.evaluate((element) => {
      const css = getComputedStyle(element)
      const control = getComputedStyle(element.querySelector('input'))
      return {
        background: css.backgroundColor,
        radius: css.borderRadius,
        outline: control.outlineColor,
        outlineStyle: control.outlineStyle,
        outlineWidth: control.outlineWidth,
      }
    })
  const restingRow = await rowRead()
  await row.hover()
  await row.evaluate((e) =>
    Promise.all(e.getAnimations().map((a) => a.finished)),
  )
  const hoverRow = await rowRead()
  await radio.click()
  const mouseRow = await rowRead()
  if (mouseRow.outlineStyle !== 'none')
    throw new Error(`Pointer ring on server radio: ${JSON.stringify(mouseRow)}`)
  await radio.focus()
  await page.keyboard.press('Tab')
  await page.keyboard.press('Shift+Tab')
  const focusRow = await rowRead()
  if (
    hoverRow.background !== 'rgb(28, 29, 32)' ||
    focusRow.outline !== 'rgb(255, 92, 57)' ||
    focusRow.outlineWidth !== '2px'
  )
    throw new Error(
      `Server row interaction failed: ${JSON.stringify({ restingRow, hoverRow, focusRow })}`,
    )
  await page.locator('input[type="radio"]').first().click()
  const reading = {
    name: 'server-row/radio',
    resting: restingRow,
    hover: hoverRow,
    mouseFocus: mouseRow,
    focus: focusRow,
  }
  await page.mouse.move(width - 1, height - 1)
  await page.evaluate(() => document.activeElement?.blur())
  return reading
}

async function verifyDiagnosticTones(fault = false) {
  const readings = await page
    .getByRole('heading', { name: 'Avanzado', exact: true })
    .evaluate((heading) => {
      const panel = heading.closest('section')
      const root = getComputedStyle(document.documentElement)
      const rgb = (name) =>
        `rgb(${root.getPropertyValue(`--c-${name}`).trim().split(/\s+/).join(', ')})`
      const labels = [
        'Runner ·',
        'Compatibilidad ·',
        'Audio ·',
        'Entorno isolated ·',
        'DXVK ·',
        'Permisos input ·',
        'Input de combate ·',
      ]
      const rows = labels.map((label) => {
        const p = [...panel.querySelectorAll('p')].find((node) =>
          node.textContent.startsWith(label),
        )
        return {
          label: p?.textContent,
          color: p
            ? getComputedStyle(p.parentElement.firstElementChild)
                .backgroundColor
            : null,
        }
      })
      return {
        rows,
        panel: getComputedStyle(panel).borderTopColor,
        width: getComputedStyle(panel).borderTopWidth,
        warn: rgb('warn'),
        bad: rgb('bad'),
        ok: rgb('ok'),
        soft: rgb('line-soft'),
      }
    })
  const tones = [
    fault ? 'bad' : 'warn',
    'warn',
    'ok',
    'warn',
    'warn',
    'ok',
    'ok',
  ]
  if (
    readings.rows.some((row, index) => row.color !== readings[tones[index]]) ||
    readings.panel !== readings.soft ||
    readings.width !== '1px'
  )
    throw new Error(`Diagnostic colors failed: ${JSON.stringify(readings)}`)
  if (
    (await page.getByRole('button', { name: 'Jugar', exact: true }).count()) !==
      0 ||
    !(await page
      .getByRole('button', { name: 'Preparar entorno', exact: true })
      .isEnabled())
  )
    throw new Error('Pending runtime changed action availability')
  diagnosticChecks.push({
    state: fault ? 'failed-runner' : 'pending-runtime',
    ...readings,
    launchAbsent: true,
    setupEnabled: true,
  })
}

async function auditBoxes() {
  return page.evaluate(() => {
    const nestedBoxes = []
    for (const element of document.querySelectorAll(
      'section div, section label, [role="dialog"] div, [role="dialog"] label',
    )) {
      if (
        element.childElementCount < 2 ||
        element.closest('button, [role="switch"]')
      )
        continue
      const css = getComputedStyle(element)
      if (
        !['Top', 'Right', 'Bottom', 'Left'].every(
          (side) => parseFloat(css[`border${side}Width`]) > 0,
        )
      )
        continue
      // Native control surfaces, modal shells and the log well are intentional boxes.
      if (
        element.matches('[role="dialog"], [data-design-log-tabs]') ||
        (element.classList.contains('font-mono') &&
          /auto|scroll/.test(css.overflowY))
      )
        continue
      nestedBoxes.push({
        panel:
          element.closest('section')?.querySelector('h2')?.textContent ??
          (element.closest('[role="dialog"]') ? 'modal' : 'rail'),
        text: element.textContent.trim().slice(0, 90),
      })
    }
    return { nestedBoxes }
  })
}

async function auditLayout() {
  return page.evaluate(() => {
    const overflow = []
    const clippedText = []
    const root = document.documentElement
    if (root.scrollWidth > innerWidth || root.scrollHeight > innerHeight)
      overflow.push('document')
    for (const element of document.querySelectorAll('body *')) {
      if (!(element instanceof HTMLElement)) continue
      const css = getComputedStyle(element)
      const rect = element.getBoundingClientRect()
      if (!rect.width || !rect.height || css.visibility === 'hidden') continue
      // Intentional scroll regions and their off-screen children are not overflow.
      let visible = true
      for (
        let parent = element.parentElement;
        parent;
        parent = parent.parentElement
      ) {
        const parentStyle = getComputedStyle(parent)
        if (
          !/(auto|scroll|hidden|clip)/.test(
            `${parentStyle.overflowX} ${parentStyle.overflowY}`,
          )
        )
          continue
        const bounds = parent.getBoundingClientRect()
        if (
          rect.bottom <= bounds.top ||
          rect.top >= bounds.bottom ||
          rect.right <= bounds.left ||
          rect.left >= bounds.right
        )
          visible = false
      }
      if (!visible) continue
      const label = element.textContent.trim().slice(0, 80)
      const positionedOverhang = [...element.children].some((child) => {
        const position = getComputedStyle(child).position
        const bounds = child.getBoundingClientRect()
        return (
          ['absolute', 'fixed'].includes(position) &&
          bounds.left >= 0 &&
          bounds.right <= innerWidth &&
          bounds.top >= 0 &&
          bounds.bottom <= innerHeight
        )
      })
      if (
        element.scrollWidth > element.clientWidth + 1 &&
        css.overflowX === 'visible' &&
        !positionedOverhang
      )
        overflow.push(`${element.tagName}: ${label}`)
      // Audit actual text-node rectangles, not scrollHeight (which rounds and
      // includes children). Fully off-screen text in a scroll region is valid.
      for (const node of element.childNodes) {
        if (node.nodeType !== Node.TEXT_NODE || !node.textContent.trim())
          continue
        const range = document.createRange()
        range.selectNodeContents(node)
        const pieces = [...range.getClientRects()]
        if (
          pieces.some(
            (piece) =>
              piece.right > rect.right + 1 || piece.left < rect.left - 1,
          )
        )
          clippedText.push(`${element.tagName}: ${label}`)
        if (
          /(hidden|clip)/.test(css.overflowY) &&
          pieces.some((piece) => piece.bottom > rect.bottom + 1)
        )
          clippedText.push(`${element.tagName}: ${label}`)
      }
      if (
        css.textOverflow === 'ellipsis' &&
        !element.closest('[title]') &&
        element.scrollWidth > element.clientWidth + 1
      )
        clippedText.push(`${element.tagName}: ${label}`)
    }
    return {
      overflow: [...new Set(overflow)],
      clippedText: [...new Set(clippedText)],
    }
  })
}

async function verifyFonts() {
  const fonts = await page.evaluate(async () => {
    const expected = [
      ...[400, 500, 600, 700].map((weight) => `${weight} 13px "IBM Plex Sans"`),
      ...[400, 500, 600].map((weight) => `${weight} 12px "IBM Plex Mono"`),
    ]
    for (const spec of expected) {
      const loaded = await document.fonts.load(spec, 'Ragnarok áéíóú ñ 123')
      if (!loaded.length || !document.fonts.check(spec))
        throw new Error(`Font not loaded: ${spec}`)
    }
    return [...document.fonts].map(({ family, weight, status }) => ({
      family,
      weight,
      status,
    }))
  })
  if (fonts.some((font) => font.status !== 'loaded'))
    throw new Error('A local font failed to load')
  const actual = await page.evaluate(() => ({
    ui: getComputedStyle(document.body).fontFamily,
    data: getComputedStyle(document.querySelector('.font-mono')).fontFamily,
    wordmark: getComputedStyle(document.querySelector('h1')).fontFamily,
  }))
  if (
    !actual.ui.includes('IBM Plex Sans') ||
    !actual.data.includes('IBM Plex Mono') ||
    !actual.wordmark.includes('IBM Plex Sans')
  )
    throw new Error('Wrong rendered font families')
  return { faces: fonts, actual }
}

async function verifyMotion() {
  await page.getByRole('button', { name: 'Ver panel', exact: true }).click()
  await page
    .getByRole('button', { name: 'Minimizar panel', exact: true })
    .waitFor()
  if (
    await page.evaluate(() =>
      document
        .getAnimations()
        .some((animation) =>
          animation.effect
            ?.getKeyframes()
            .some((frame) => frame.transform && frame.transform !== 'none'),
        ),
    )
  )
    throw new Error('Rail moved with an animated transform')
  await page
    .getByRole('button', { name: 'Minimizar panel', exact: true })
    .click()
  await page.emulateMedia({ reducedMotion: 'reduce' })
  const reduced = await page.evaluate(() =>
    [...document.querySelectorAll('.animate-pulse-dot')].every(
      (element) =>
        parseFloat(getComputedStyle(element).animationDuration) <= 0.00001,
    ),
  )
  if (!reduced) throw new Error('Reduced motion is not respected')
  await page.emulateMedia({ reducedMotion: 'no-preference' })
}

async function verifySwitchContrast(state) {
  const readings = await page.evaluate(() => {
    const luminance = (color) => {
      const channels = color
        .match(/[\d.]+/g)
        .slice(0, 3)
        .map(Number)
      return channels
        .map((channel) => channel / 255)
        .map((channel) =>
          channel <= 0.04045
            ? channel / 12.92
            : ((channel + 0.055) / 1.055) ** 2.4,
        )
        .reduce(
          (sum, channel, index) =>
            sum + channel * [0.2126, 0.7152, 0.0722][index],
          0,
        )
    }
    const contrast = (a, b) =>
      (Math.max(luminance(a), luminance(b)) + 0.05) /
      (Math.min(luminance(a), luminance(b)) + 0.05)
    return [...document.querySelectorAll('[role="switch"]')].map((element) => {
      const css = getComputedStyle(element)
      const panel = getComputedStyle(element.closest('section'))
      const knob = getComputedStyle(element.firstElementChild)
      let opacity = 1
      for (let ancestor = element; ancestor; ancestor = ancestor.parentElement)
        opacity *= Number(getComputedStyle(ancestor).opacity)
      return {
        checked: element.getAttribute('aria-checked'),
        disabled: element.disabled,
        opacity,
        border: css.borderTopColor,
        track: css.backgroundColor,
        knob: knob.backgroundColor,
        boundaryKind:
          element.getAttribute('aria-checked') === 'false'
            ? 'decorativo'
            : 'estado',
        boundaryContrast: contrast(
          css.outlineStyle !== 'none' && parseFloat(css.outlineWidth)
            ? css.outlineColor
            : parseFloat(css.borderTopWidth) && css.borderTopStyle !== 'none'
              ? css.borderTopColor
              : css.backgroundColor,
          panel.backgroundColor,
        ),
        knobContrast: contrast(knob.backgroundColor, css.backgroundColor),
      }
    })
  })
  if (!readings.some((reading) => reading.checked === String(state === 'on')))
    throw new Error(`No ${state} switch rendered`)
  if (
    readings.some(
      (reading) =>
        reading.opacity !== 1 ||
        (reading.boundaryKind !== 'decorativo' &&
          reading.boundaryContrast < 3) ||
        reading.knobContrast < 3,
    )
  )
    contrastFailures.push({ kind: 'switch-boundary', state, readings })
  switchContrast.push({ state, readings })
}

async function auditStateContrast() {
  const result = await page.evaluate(() => {
    const rgba = (value) => {
      const channels = value.match(/[\d.]+/g).map(Number)
      return [
        ...channels
          .slice(0, 3)
          .map((channel) =>
            value.startsWith('color(srgb') ? channel * 255 : channel,
          ),
        channels[3] ?? 1,
      ]
    }
    const blend = (foreground, background) =>
      foreground
        .slice(0, 3)
        .map(
          (channel, index) =>
            channel * foreground[3] + background[index] * (1 - foreground[3]),
        )
    const background = (element) => {
      const ancestors = []
      for (let node = element; node; node = node.parentElement)
        ancestors.unshift(node)
      return ancestors.reduce(
        (color, node) =>
          blend(rgba(getComputedStyle(node).backgroundColor), color),
        [0, 0, 0],
      )
    }
    const luminance = (color) =>
      color
        .map((channel) => channel / 255)
        .map((channel) =>
          channel <= 0.04045
            ? channel / 12.92
            : ((channel + 0.055) / 1.055) ** 2.4,
        )
        .reduce(
          (sum, channel, index) =>
            sum + channel * [0.2126, 0.7152, 0.0722][index],
          0,
        )
    const contrast = (a, b) =>
      (Math.max(luminance(a), luminance(b)) + 0.05) /
      (Math.min(luminance(a), luminance(b)) + 0.05)
    const opacity = (element) => {
      let result = 1
      for (let node = element; node; node = node.parentElement)
        result *= Number(getComputedStyle(node).opacity)
      return result
    }
    const idle = [
      ...document.querySelectorAll(
        '.panel-idle h2, .panel-idle .text-muted, .panel-idle .text-ink',
      ),
    ]
      .filter(
        (element) =>
          element.getBoundingClientRect().height && element.textContent.trim(),
      )
      .map((element) => {
        const bg = background(element)
        return {
          text: element.textContent.trim().slice(0, 65),
          opacity: opacity(element),
          textContrast: contrast(
            blend(rgba(getComputedStyle(element).color), bg),
            bg,
          ),
        }
      })
    const idleBorders = [...document.querySelectorAll('.panel-idle')].map(
      (element) => {
        const css = getComputedStyle(element)
        return {
          panel: element.querySelector('h2').textContent,
          kind: 'decorativo',
          contrast: contrast(
            blend(rgba(css.borderTopColor), background(element)),
            background(element),
          ),
        }
      },
    )
    const disabled = [...document.querySelectorAll(':disabled')]
      .filter((element) => element.getBoundingClientRect().height)
      .map((element) => {
        const css = getComputedStyle(element)
        const bg = background(element)
        const outside = background(element.parentElement)
        const boundary =
          css.outlineStyle !== 'none' && parseFloat(css.outlineWidth)
            ? css.outlineColor
            : css.borderTopColor
        const choice = element.matches(
          'input[type="radio"], input[type="checkbox"], [role="checkbox"]',
        )
        const hasText =
          !!(
            element.textContent.trim() ||
            element.value ||
            element.getAttribute('placeholder')
          ) &&
          !element.matches('[type="range"], [type="radio"], [type="checkbox"]')
        return {
          tag: element.tagName,
          type: element.type,
          role: element.getAttribute('role'),
          label:
            element.getAttribute('aria-label') ??
            element.textContent.trim().slice(0, 65),
          opacity: opacity(element),
          color: css.color,
          background: css.backgroundColor,
          border: css.borderTopColor,
          outline: css.outlineColor,
          outlineWidth: css.outlineWidth,
          textContrast: contrast(blend(rgba(css.color), bg), bg),
          textMinimum: hasText ? 4.5 : 0,
          boundaryMinimum: choice ? 3 : 0,
          boundaryKind: choice ? 'estado' : 'decorativo',
          innerBorderContrast: contrast(
            blend(rgba(css.borderTopColor), bg),
            bg,
          ),
          boundaryContrast: contrast(blend(rgba(boundary), outside), outside),
          boundaryInsideContrast: contrast(blend(rgba(boundary), bg), bg),
        }
      })
    const enabled = [
      ...document.querySelectorAll(
        'button:not(:disabled), input:not(:disabled), textarea:not(:disabled), summary',
      ),
    ]
      .filter((element) => element.getBoundingClientRect().height)
      .map((element) => {
        const css = getComputedStyle(element)
        const bg = background(element)
        const outside = background(element.parentElement)
        const hasText = !!(
          element.textContent.trim() ||
          element.value ||
          element.getAttribute('placeholder')
        )
        const paintedIcon =
          element.querySelector('svg') &&
          !element.matches('[role="checkbox"][aria-checked="false"]')
        const isField = element.matches(
          'input:not([type="range"]):not([type="radio"]):not([type="checkbox"]), textarea, [aria-haspopup="listbox"]',
        )
        const isChoice = element.matches(
          'input[type="radio"], input[type="checkbox"], [role="checkbox"]',
        )
        const border = rgba(css.borderTopColor)
        return {
          label:
            element.getAttribute('aria-label') ??
            element.textContent.trim().slice(0, 65) ??
            element.type,
          textContrast: contrast(blend(rgba(css.color), bg), bg),
          textMinimum:
            hasText &&
            !element.matches(
              '[type="range"], [type="radio"], [type="checkbox"]',
            )
              ? 4.5
              : paintedIcon
                ? 3
                : 0,
          opacity: opacity(element),
          borderContrast:
            isField || isChoice
              ? contrast(blend(border, outside), outside)
              : null,
          boundaryKind: isField ? 'decorativo' : 'estado',
        }
      })
    const notices = [...document.querySelectorAll('.notice-warn')].map(
      (element) => ({
        text: element.textContent.trim().slice(0, 65),
        textContrast: contrast(
          blend(rgba(getComputedStyle(element).color), background(element)),
          background(element),
        ),
      }),
    )
    const points = [
      ...document.querySelectorAll(
        'span.bg-ok, span.bg-warn, span.bg-bad, span.bg-info, span.bg-muted, span.bg-line-strong',
      ),
    ]
      .filter((element) => element.getBoundingClientRect().width === 8)
      .map((element) => {
        const css = getComputedStyle(element)
        const bg = background(element.parentElement)
        return {
          color: css.backgroundColor,
          opacity: opacity(element),
          contrast: contrast(blend(rgba(css.backgroundColor), bg), bg),
        }
      })
    return { idle, disabled, idleBorders, enabled, notices, points }
  })
  if (
    result.idle.some(
      (reading) => reading.opacity !== 1 || reading.textContrast < 4.5,
    ) ||
    result.disabled.some(
      (reading) =>
        reading.opacity !== 1 ||
        reading.textContrast < reading.textMinimum ||
        reading.boundaryContrast < reading.boundaryMinimum ||
        reading.boundaryInsideContrast < reading.boundaryMinimum,
    )
  )
    throw new Error(`Idle/disabled contrast failed: ${JSON.stringify(result)}`)
  const textFailures = result.enabled.filter(
    (reading) =>
      reading.opacity !== 1 || reading.textContrast < reading.textMinimum,
  )
  if (
    textFailures.length ||
    result.notices.some((reading) => reading.textContrast < 4.5)
  )
    throw new Error(
      `Rendered text/tonal contrast failed: ${JSON.stringify({ textFailures, notices: result.notices })}`,
    )
  const borders = result.enabled.filter(
    (reading) =>
      reading.boundaryKind !== 'decorativo' &&
      reading.borderContrast !== null &&
      reading.borderContrast < 3,
  )
  if (result.points.some((point) => point.opacity !== 1 || point.contrast < 3))
    throw new Error(
      `Status point contrast failed: ${JSON.stringify(result.points)}`,
    )
  if (borders.length)
    contrastFailures.push({
      kind: 'field-choice-boundary',
      scene: checks.at(-1).scene,
      readings: borders,
    })
  return result
}

await page.goto(`http://127.0.0.1:${port}/?fixture=standard`)
await page.getByRole('button', { name: 'Editar TestRO' }).waitFor()
if (!baseline) controlCheck = await verifyQuietControls()
await capture('prep')
if (!baseline) await verifySwitchContrast('off')
await page.getByRole('button', { name: 'Editar TestRO' }).click()
await page.getByRole('dialog', { name: 'Editar servidor' }).waitFor()
await capture('server')
await page.getByRole('button', { name: 'Cerrar configuración' }).click()
await page.evaluate(() => {
  window.__designEmit('ro-launcher://game-client', {
    clientId: 'design-client',
    serverId: 'design-fixture',
    serverName: 'TestRO',
    status: 'running',
    pid: 42,
    memoryAccess: 'processVmReadv',
    profileMemory: 'valid',
  })
})
await page.getByText('En juego', { exact: false }).first().waitFor()
await capture('ingame')
if (!baseline) await verifySwitchContrast('off')
await page.getByRole('button', { name: 'Encontrar', exact: true }).click()
await page
  .getByRole('dialog', { name: 'Encontrar memoria de TestRO' })
  .waitFor()
await capture('scanner')
await page.getByRole('button', { name: 'Cancelar', exact: true }).click()
await page
  .locator('section')
  .filter({ has: page.getByRole('heading', { name: 'AutoPot', exact: true }) })
  .getByRole('switch')
  .first()
  .click()
await page
  .locator('section')
  .filter({ has: page.getByRole('heading', { name: 'Spammer', exact: true }) })
  .getByRole('switch')
  .first()
  .click()
await page.evaluate(() => {
  window.__designEmit('ro-launcher://autopot-status', {
    active: true,
    effectiveDelayMs: 100,
    curHp: 1200,
    maxHp: 2000,
    curSp: 400,
    maxSp: 800,
    characterName: 'Test',
    memoryAccess: 'processVmReadv',
    hpPercent: 80,
    spPercent: 50,
    error: null,
  })
  window.__designEmit('ro-launcher://spammer-status', {
    active: true,
    effectiveDelayMs: 16,
    armed: true,
    spamming: true,
    key: 'F1',
    delayMs: 16,
    cycleCount: 0,
    error: null,
  })
})
await page.getByText('Test', { exact: true }).waitFor()
await capture('active')
if (!baseline) await verifySwitchContrast('on')
await page
  .getByRole('button', {
    name: 'Sharp Shooting / Focused Arrow Strike',
    exact: false,
  })
  .click()
await page
  .getByRole('button', { name: 'ATK / DEF Gear Switch', exact: false })
  .click()
await capture('active-editors')
if (!baseline) await verifySwitchContrast('on')
await page.getByRole('button', { name: 'Buffs', exact: true }).click()
await page.getByRole('heading', { name: 'AutoBuff', exact: true }).waitFor()
await page
  .getByRole('button', { name: '+ Concentration Potion', exact: true })
  .click()
await page
  .getByRole('button', { name: '+ Awakening Potion', exact: true })
  .click()
await capture('buffs')
if (!baseline) await verifySwitchContrast('off')
await page.getByRole('button', { name: 'Combate', exact: true }).click()
const fonts = await verifyFonts()
await verifyMotion()

await page.goto(`http://127.0.0.1:${port}/?fixture=realistic`)
await page.getByRole('button', { name: 'Editar TestRO' }).waitFor()
await page.getByText('HoneyRO Patcher.exe', { exact: true }).waitFor()
if (
  (await page.locator('details').getAttribute('open')) !== null ||
  (await page
    .getByRole('button', { name: 'Adjuntar', exact: true })
    .isVisible())
)
  throw new Error('Review disclosure must start closed')
await page.evaluate(() => {
  for (const line of [
    'Runtime administrado nndsk-ro-proton verificado; prefix aislado /fixture/prefixes/honeyro/nndsk-ro-proton-verified-isolated-prefix; DXVK 2.6.2 disponible.',
    'Diagnóstico: cliente x86 d3d9, OpenSetup Setup.exe y HoneyRO Patcher.exe encontrados; configuración dgVoodoo conf OK, sin cambios en los ejecutables.',
    'Observación local de prueba: la identidad del proceso se valida antes de cada operación; no se modifican archivos del juego ni políticas del host.',
    'Preparación completada para la revisión visual; las líneas largas de logs conservan el contenido completo y se ajustan al ancho del pozo de logs.',
  ])
    window.__designEmit('ro-launcher://log', { line })
})
await page
  .getByText('Preparación completada para la revisión visual', { exact: false })
  .waitFor()
await capture('prep-realistic')
if (!baseline) await verifySwitchContrast('off')
const railScroll = page
  .locator('main > div')
  .first()
  .locator('.overflow-y-auto')
  .first()
const pinnedBefore = await Promise.all(
  ['Jugar', 'Rearmar entorno'].map((name) =>
    page.getByRole('button', { name, exact: true }).boundingBox(),
  ),
)
await railScroll.evaluate((element) => {
  element.scrollTop = element.scrollHeight
})
await capture('prep-realistic-scrolled')
await page.locator('summary').filter({ hasText: 'Benchmarks A/B' }).click()
await page.getByRole('button', { name: 'Adjuntar', exact: true }).waitFor()
await capture('prep-realistic-open')
const actions = await page
  .locator('details button:not([aria-haspopup])')
  .evaluateAll((elements) =>
    elements.map((element) => ({
      text: element.textContent.trim(),
      disabled: element.disabled,
    })),
  )
const expectedActions = [
  ['Exportar observaciones', false],
  ['Borrar', false],
  ['Adjuntar', true],
  ['Iniciar captura', true],
  ['Terminar captura', true],
  ['Importar CSV', true],
  ['Visual OK', true],
  ['Visual fallo', true],
  ['Visual omitir', true],
  ['Comparar', true],
  ['Exportar benchmarks', false],
  ['Exportar comparación', true],
  ['Borrar benchmarks', false],
].map(([text, disabled]) => ({ text, disabled }))
if (JSON.stringify(actions) !== JSON.stringify(expectedActions))
  throw new Error('Review actions changed order or availability')
disclosureCheck = { startsClosed: true, actions }
await railScroll.evaluate((element) => {
  element.scrollTop = element.scrollHeight
})
await capture('prep-realistic-open-scrolled')
const armSelect = page.getByRole('combobox')
await armSelect.evaluate((element) => {
  const rail = element.closest('[data-design-rail-scroll]')
  rail.scrollTop +=
    element.getBoundingClientRect().bottom -
    (rail.getBoundingClientRect().bottom - 8)
})
await armSelect.click()
await page.getByRole('listbox').waitFor()
await capture('advanced-select')
const menuBounds = await page.getByRole('listbox').boundingBox()
if (
  !menuBounds ||
  menuBounds.x < 0 ||
  menuBounds.y < 0 ||
  menuBounds.x + menuBounds.width > width ||
  menuBounds.y + menuBounds.height > height
)
  throw new Error('DarkSelect menu escaped the viewport')
await page.setViewportSize({ width: width + 80, height: height + 40 })
const resizedMenu = await page.getByRole('listbox').boundingBox()
if (!resizedMenu || resizedMenu.y + resizedMenu.height > height + 40)
  throw new Error('DarkSelect did not reposition on resize')
await page.setViewportSize({ width, height })
await page.getByRole('option', { name: 'Brazo B', exact: true }).click()
if ((await armSelect.textContent()) !== 'Brazo B')
  throw new Error('DarkSelect did not preserve selection')
await armSelect.press('ArrowDown')
await page.getByRole('option', { name: 'Brazo B', exact: true }).press('Home')
await page.getByRole('option', { name: 'Brazo A', exact: true }).press('Enter')
if ((await armSelect.textContent()) !== 'Brazo A')
  throw new Error('DarkSelect keyboard selection failed')
selectCheck = {
  withinViewport: true,
  repositionsOnResize: true,
  pointerAndKeyboardSelection: true,
}
await page.locator('summary').filter({ hasText: 'Benchmarks A/B' }).click()
if (
  await page.getByRole('button', { name: 'Adjuntar', exact: true }).isVisible()
)
  throw new Error('Disclosure did not close')
disclosureCheck.closesLocally = true
if (!baseline) {
  const pinnedAfter = await Promise.all(
    ['Jugar', 'Rearmar entorno'].map((name) =>
      page.getByRole('button', { name, exact: true }).boundingBox(),
    ),
  )
  if (
    pinnedAfter.some(
      (bounds, index) =>
        !bounds ||
        bounds.y !== pinnedBefore[index]?.y ||
        bounds.height !== pinnedBefore[index]?.height,
    )
  )
    throw new Error('Bottom actions moved with the rail scroll')
  railCheck = await railScroll.evaluate((element) => ({
    scrollHeight: element.scrollHeight,
    clientHeight: element.clientHeight,
    nestedScrolls: [...element.querySelectorAll('*')].filter((child) =>
      /auto|scroll/.test(getComputedStyle(child).overflowY),
    ).length,
  }))
  if (
    railCheck.scrollHeight <= railCheck.clientHeight ||
    railCheck.nestedScrolls
  )
    throw new Error(
      `Rail needs one scroll region: ${JSON.stringify(railCheck)}`,
    )
  railCheck.bottomActionsFixed = true
}
await page.goto(`http://127.0.0.1:${port}/?fixture=pending`)
await page.getByText('DXVK · pendiente', { exact: true }).waitFor()
if (!baseline) await verifyDiagnosticTones()
if (!baseline) controlCheck.push(await verifyServerRowInteraction())
await capture('prep-pending')
await page.locator('[data-design-rail-scroll]').evaluate((element) => {
  element.scrollTop = element.scrollHeight
})
await capture('prep-pending-scrolled')
if (!baseline) {
  await page.goto(`http://127.0.0.1:${port}/?fixture=pending&fault=runner`)
  await page.getByText('DXVK · pendiente', { exact: true }).waitFor()
  await verifyDiagnosticTones(true)
}

await page.goto(`http://127.0.0.1:${port}/?fixture=empty`)
await page.getByText('Sin servidores — agrega uno con +').waitFor()
await capture('prep-empty')
if (!baseline) await verifySwitchContrast('off')

await page.goto(`http://127.0.0.1:${port}/?fixture=fit-combat`)
await page
  .getByRole('button', { name: 'Editar SakuraRO', exact: true })
  .waitFor()
await page.getByText('HoneyRO Patcher.exe', { exact: true }).waitFor()
await page.evaluate(() => {
  for (const line of [
    'Diagnóstico local: cliente DirectDraw y Direct3D 9; prefix aislado /fixture/prefixes/sakuraro/wine-7.16-staging-tkg-amd64; no se modifican ejecutables ni anti-cheat.',
    'OpenSetup /fixture/Setup.exe y patcher /fixture/HoneyRO Patcher.exe encontrados; configuración dgVoodoo conf OK. Las rutas completas se conservan para revisión.',
    'Revisión de encaje completada; las líneas largas de logs siguen accesibles en su pozo, sin scroll anidado ni solape con las reglas de AutoBuff.',
  ])
    window.__designEmit('ro-launcher://log', { line })
})
await page
  .getByText('Revisión de encaje completada', { exact: false })
  .waitFor()
await capture('prep-combat-fit')
if (!baseline) {
  for (const title of ['Runner predeterminado', 'Herramientas']) {
    const panel = page
      .locator('section')
      .filter({ has: page.getByRole('heading', { name: title, exact: true }) })
    const notice = panel.locator('.notice-warn')
    const collapsed = await notice.boundingBox()
    const content = await notice.textContent()
    await panel
      .getByRole('button', { name: 'Mostrar más', exact: true })
      .click()
    if ((await notice.textContent()) !== content)
      throw new Error('Notice text changed on expansion')
    const expanded = await notice.boundingBox()
    await capture(
      `fit-notice-${title === 'Herramientas' ? 'tools' : 'runner'}-expanded`,
    )
    await panel
      .getByRole('button', { name: 'Mostrar menos', exact: true })
      .click()
    fitDetails.notices.push({
      panel: title,
      collapsed: collapsed.height,
      expanded: expanded.height,
      gained: expanded.height - collapsed.height,
    })
  }
  const rail = page.locator('[data-design-rail-scroll]')
  await page
    .getByRole('heading', { name: 'Avanzado', exact: true })
    .scrollIntoViewIfNeeded()
  await capture('fit-advanced-reachable')
  await rail.evaluate((element) => {
    element.scrollTop = 0
  })
}
await page.getByRole('button', { name: 'Buffs', exact: true }).click()
await page.getByRole('heading', { name: 'AutoBuff', exact: true }).waitFor()
await capture('prep-buffs')
if (!baseline) {
  const buffPanel = page.locator('section').filter({
    has: page.getByRole('heading', { name: 'AutoBuff', exact: true }),
  })
  await buffPanel
    .getByRole('button', { name: 'Mostrar catálogo', exact: true })
    .click()
  await buffPanel
    .getByRole('button', { name: '+ Manual', exact: true })
    .scrollIntoViewIfNeeded()
  if (
    (await buffPanel
      .getByRole('button', { name: '+ Manual', exact: true })
      .isVisible()) !== true
  )
    throw new Error('Manual preset is inaccessible')
  await capture('prep-buffs-catalog')
  await buffPanel
    .getByRole('button', { name: 'Ocultar catálogo', exact: true })
    .click()
  const scroll = buffPanel.locator('[data-design-panel-scroll]')
  const rows = buffPanel.locator('[data-design-buff-row]')
  const statusBefore = await buffPanel
    .getByText('Sin buffs aplicados', { exact: true })
    .boundingBox()
  for (const row of await rows.all()) {
    await row.evaluate((element) => element.scrollIntoView({ block: 'center' }))
    const fits = await row.evaluate((element) => {
      const body = element
        .closest('[data-design-panel-scroll]')
        .getBoundingClientRect()
      const bounds = element.getBoundingClientRect()
      return bounds.top >= body.top - 1 && bounds.bottom <= body.bottom + 1
    })
    if (!fits) throw new Error('A buff row cannot be shown at full height')
  }
  const sticky = await buffPanel
    .getByText('Cooldown', { exact: true })
    .boundingBox()
  const bodyBounds = await scroll.boundingBox()
  const statusAfter = await buffPanel
    .getByText('Sin buffs aplicados', { exact: true })
    .boundingBox()
  if (
    Math.abs(statusAfter.y - statusBefore.y) > 1 ||
    sticky.y < bodyBounds.y - 1 ||
    sticky.y > bodyBounds.y + 10
  )
    throw new Error('Buff status/table header moved with rows')
  fitDetails.buffs.push({
    scene: 'prep',
    rules: await rows.count(),
    allRowsReachable: true,
    statusFixed: true,
    headerSticky: true,
  })
  await scroll.evaluate((element) => {
    element.scrollTop = 0
  })
}
await page
  .locator('section')
  .filter({ has: page.getByRole('heading', { name: 'AutoBuff', exact: true }) })
  .locator('[aria-haspopup="listbox"]')
  .first()
  .click({ force: baseline })
await page.getByRole('listbox').waitFor()
await capture('prep-buffs-select')
await page.keyboard.press('Escape')

await page.goto(`http://127.0.0.1:${port}/?fixture=fit-ingame`)
await page
  .getByRole('button', { name: 'Editar SakuraRO', exact: true })
  .waitFor()
await page.evaluate(() =>
  window.__designEmit('ro-launcher://game-client', {
    clientId: 'design-client',
    serverId: 'design-fixture',
    serverName: 'SakuraRO',
    status: 'running',
    pid: 42,
    memoryAccess: 'processVmReadv',
    profileMemory: 'valid',
  }),
)
await page.getByRole('button', { name: 'Buffs', exact: true }).click()
await page.getByRole('heading', { name: 'AutoBuff', exact: true }).waitFor()
await capture('ingame-buffs')

await page.goto(`http://127.0.0.1:${port}/?fixture=fit-starting`)
await page.getByRole('button', { name: 'Jugar', exact: true }).waitFor()
await page.getByRole('button', { name: 'Buffs', exact: true }).click()
await page.getByRole('button', { name: 'Jugar', exact: true }).click()
await page.getByText('Iniciando juego...', { exact: true }).first().waitFor()
await capture('ingame-buffs-starting')

await page.goto(`http://127.0.0.1:${port}/?fixture=fit-ready`)
await page
  .getByRole('button', { name: 'Editar SakuraRO', exact: true })
  .waitFor()
await page.getByText('HoneyRO Patcher.exe', { exact: true }).waitFor()
await capture('prep-ready')
if (!baseline) {
  const tools = page.locator('section').filter({
    has: page.getByRole('heading', { name: 'Herramientas', exact: true }),
  })
  for (const name of ['Abrir', 'Config'])
    for (const button of await tools
      .getByRole('button', { name, exact: true })
      .all()) {
      if (await button.isDisabled())
        throw new Error(`Ready tool is disabled: ${name}`)
      const reading = stateContrast
        .at(-1)
        .enabled.find((reading) => reading.label === name)
      if (!reading || reading.textContrast < 4.5)
        throw new Error(`Ready tool contrast: ${name}`)
      fitDetails.readyActions.push({
        name,
        enabled: true,
        contrast: reading.textContrast,
      })
    }
}
// The approved HTML is unchanged. Load the same local font faces rather than
// its example CDN, then capture only its first (preparation) board.
if (width === 1440 && height === 900 && !baseline) {
  const reference = await browser.newPage({ viewport: { width, height } })
  await reference.route('https://fonts.googleapis.com/**', (route) =>
    route.fulfill({ body: '', contentType: 'text/css' }),
  )
  await reference.goto(
    `file://${path.resolve('docs/design-reference/refined.html')}`,
  )
  const faces = []
  for (const [family, directory, weights] of [
    ['IBM Plex Sans', 'ibm-plex-sans', [400, 500, 600]],
    ['IBM Plex Mono', 'ibm-plex-mono', [400, 500]],
  ]) {
    for (const weight of weights) {
      const data = fs
        .readFileSync(
          `node_modules/@fontsource/${directory}/files/${directory}-latin-${weight}-normal.woff2`,
        )
        .toString('base64')
      faces.push(
        `@font-face{font-family:${JSON.stringify(family)};font-weight:${weight};src:url(data:font/woff2;base64,${data}) format("woff2");}`,
      )
    }
  }
  await reference.addStyleTag({ content: faces.join('\n') })
  await reference.evaluate(() => document.fonts.ready)
  const refImage = await reference.locator('body > div').first().screenshot()
  const appImage = fs.readFileSync(
    path.join(output, 'prep-combat-fit-1440x900.png'),
  )
  const comparison = await browser.newPage({
    viewport: { width: width * 2, height },
  })
  await comparison.setContent(
    `<body style="margin:0;display:flex"><img width="1440" height="900" src="data:image/png;base64,${refImage.toString('base64')}"><img width="1440" height="900" src="data:image/png;base64,${appImage.toString('base64')}"></body>`,
  )
  await comparison
    .locator('img')
    .evaluateAll((images) => Promise.all(images.map((image) => image.decode())))
  await comparison.screenshot({
    path: path.join(output, 'reference-vs-app-1440x900.png'),
  })
  conformance = await reviewConformance({ reference, page, port, output })
  process.stdout.write(
    `Unjustified conformance deviations: ${conformance.failures.length}\n`,
  )
  await reference.close()
  await comparison.close()
}
process.stdout.write(
  `Review ${width}x${height}: ${checks.length} scenes, ${errors.length} console/page errors, ${checks.reduce((total, check) => total + check.fit.failures.length, 0)} fit failures, ${contrastFailures.length} contrast failures, ${fonts.faces.filter((face) => face.status === 'loaded').length} font faces loaded\n`,
)
fs.writeFileSync(
  output === path.resolve('docs/design-review')
    ? `/tmp/ro-soft-fit-review-${width}x${height}.json`
    : path.join(output, `review-${width}x${height}.json`),
  JSON.stringify(
    {
      viewport: { width, height },
      csp,
      errors,
      checks,
      fonts,
      fontResponses,
      switchContrast,
      stateContrast,
      selectCheck,
      disclosureCheck,
      railCheck,
      controlCheck,
      diagnosticChecks,
      contrastFailures,
      conformance,
      fitDetails,
    },
    null,
    2,
  ) + '\n',
)
await browser.close()
if (
  contrastFailures.length ||
  conformance?.failures.length ||
  errors.length ||
  checks.some(
    (check) =>
      check.overflow.length ||
      check.clippedText.length ||
      check.fit.failures.length ||
      (!baseline && check.nestedBoxes.length),
  ) ||
  fontResponses.some((response) => response.status !== 200)
)
  process.exitCode = 1
