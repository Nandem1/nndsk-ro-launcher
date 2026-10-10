import fs from 'node:fs'
import path from 'node:path'
import { chromium } from 'playwright'

const output = path.resolve('docs/design-review')
const port = process.argv[2] ?? '5175'
const width = Number(process.argv[3] ?? 1280)
const height = Number(process.argv[4] ?? 820)
const executablePath = process.env.RO_DESIGN_CHROMIUM ?? '/usr/bin/chromium'
fs.mkdirSync(output, { recursive: true })
const csp = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf8')).app
  .security.csp
const checks = []
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
await page.route(`http://127.0.0.1:${port}/`, async (route) => {
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
  const server = {
    id: 'design-fixture',
    name: 'TestRO',
    executablePath: '/fixture/ragexe.exe',
    prefixMode: 'isolated',
  }
  const runner = {
    id: 'nndsk-ro-proton-0.1.0-dev.2',
    name: 'nndsk-ro-proton',
    path: '/fixture/proton',
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
    prefixPath: '/fixture/prefix',
    prefixScope: 'isolated',
    prefixManaged: true,
    readyToLaunch: true,
    canSetup: true,
    canReset: true,
    checks: [],
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
  const responses = {
    list_servers: [server],
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
  checks.push({ scene: name, ...(await auditLayout()) })
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
    `/tmp/ro-unslop-${width}x${height}-${name}.json`,
    JSON.stringify(styles, null, 2),
  )
  await page.screenshot({
    path: path.join(output, `${name}-${width}x${height}.png`),
  })
  console.log(name, styles.length, 'elements rendered')
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
      '700 18px "Barlow Condensed"',
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
    !actual.wordmark.includes('Barlow Condensed')
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

await page.goto(`http://127.0.0.1:${port}/`)
await page.getByRole('button', { name: 'Editar TestRO' }).waitFor()
await capture('prep')
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
const fonts = await verifyFonts()
await verifyMotion()
console.log('Console/page errors:', errors)
console.log('Layout checks:', checks)
console.log('Loaded fonts:', fonts)
fs.writeFileSync(
  path.join(output, `review-${width}x${height}.json`),
  JSON.stringify(
    { viewport: { width, height }, csp, errors, checks, fonts, fontResponses },
    null,
    2,
  ) + '\n',
)
await browser.close()
if (
  errors.length ||
  checks.some((check) => check.overflow.length || check.clippedText.length) ||
  fontResponses.some((response) => response.status !== 200)
)
  process.exitCode = 1
