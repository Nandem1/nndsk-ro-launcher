#!/usr/bin/env node
// Drives one isolated RO-Launcher `tauri dev` webview through WebKit's inspector.
// Never matches windows by title: the installed AppImage uses the same title.

import { spawn, execFileSync } from 'node:child_process'
import { createConnection } from 'node:net'
import {
  closeSync,
  existsSync,
  mkdirSync,
  openSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { setTimeout as delay } from 'node:timers/promises'

const scriptDir = dirname(fileURLToPath(import.meta.url))
const repoRoot = resolve(scriptDir, '../../../..')
const root = '/tmp/ro-launcher-verify'
const statePath = `${root}/run.json`
const evidenceRoot = `${root}/evidence`
const logPath = `${root}/tauri.log`
const inspectorHost = '127.0.0.1'
const inspectorPort = 9222
const vitePort = 5173
const readyText = 'Ragnarok Online'

function fail(message) {
  console.error(message)
  process.exit(1)
}

function readState() {
  if (!existsSync(statePath)) fail(`No hay una instancia de verificación (${statePath}).`)
  return JSON.parse(readFileSync(statePath, 'utf8'))
}

function pidAlive(pid) {
  try {
    process.kill(pid, 0)
    return true
  } catch {
    return false
  }
}

function portOpen(port) {
  return new Promise((resolvePort) => {
    const socket = createConnection({ port, host: '127.0.0.1' }, () => {
      socket.end()
      resolvePort(true)
    })
    socket.on('error', () => resolvePort(false))
  })
}

function childPids(pid) {
  try {
    const out = execFileSync('ps', ['-o', 'pid=', '--ppid', String(pid)], {
      encoding: 'utf8',
    })
    return out
      .split('\n')
      .map((line) => Number(line.trim()))
      .filter((value) => Number.isInteger(value) && value > 0)
  } catch {
    return []
  }
}

function processTree(pid, seen = new Set()) {
  if (!pid || seen.has(pid)) return seen
  seen.add(pid)
  for (const child of childPids(pid)) processTree(child, seen)
  return seen
}

function listenerPid(port) {
  try {
    const out = execFileSync('ss', ['-ltnp'], { encoding: 'utf8' })
    for (const line of out.split('\n')) {
      if (!line.includes(`:${port}`) || !line.includes('pid=')) continue
      const match = line.match(/pid=(\d+)/)
      if (match) return Number(match[1])
    }
  } catch {
    return null
  }
  return null
}

function environOf(pid) {
  try {
    return readFileSync(`/proc/${pid}/environ`).toString('utf8').split('\0')
  } catch {
    return []
  }
}

function envValue(pid, key) {
  const prefix = `${key}=`
  return environOf(pid).find((entry) => entry.startsWith(prefix))?.slice(prefix.length) ?? ''
}

async function inspectorPage() {
  const response = await fetch(`http://${inspectorHost}:${inspectorPort}/`)
  if (!response.ok) throw new Error(`El inspector respondió ${response.status}`)
  const html = await response.text()
  const match = html.match(/class="targeturl">([^<]+)<\/div>[\s\S]*?(\/socket\/\d+\/\d+\/WebPage)/)
  if (!match) throw new Error('El inspector no publicó un objetivo WebPage')
  const url = match[1]
  if (!url.includes(`:${vitePort}`)) {
    throw new Error(`El objetivo del inspector no es Vite :${vitePort} (${url})`)
  }
  return { url, socketPath: match[2] }
}

function connectInspector(socketPath) {
  const ws = new WebSocket(`ws://${inspectorHost}:${inspectorPort}${socketPath}`)
  let outerId = 0
  let innerId = 0
  const outerPending = new Map()
  const innerPending = new Map()
  let pageTargetId = ''
  const targetWaiters = []

  ws.addEventListener('message', (event) => {
    const message = JSON.parse(String(event.data))
    if (message.id && outerPending.has(message.id)) {
      const pending = outerPending.get(message.id)
      outerPending.delete(message.id)
      if (message.error) pending.reject(new Error(JSON.stringify(message.error)))
      else pending.resolve(message.result ?? {})
    }
    if (message.method === 'Target.targetCreated' && message.params?.targetInfo?.type === 'page') {
      pageTargetId = message.params.targetInfo.targetId
      for (const resolveTarget of targetWaiters.splice(0)) resolveTarget(pageTargetId)
    }
    if (message.method === 'Target.dispatchMessageFromTarget') {
      const inner = JSON.parse(message.params.message)
      if (!inner.id || !innerPending.has(inner.id)) return
      const pending = innerPending.get(inner.id)
      innerPending.delete(inner.id)
      if (inner.error) pending.reject(new Error(JSON.stringify(inner.error)))
      else pending.resolve(inner.result ?? {})
    }
  })

  const opened = new Promise((resolveOpen, rejectOpen) => {
    ws.addEventListener('open', () => resolveOpen())
    ws.addEventListener('error', () => rejectOpen(new Error(`No se pudo abrir ${socketPath}`)))
  })

  function sendOuter(method, params) {
    const id = ++outerId
    return new Promise((resolveResult, rejectResult) => {
      outerPending.set(id, { resolve: resolveResult, reject: rejectResult })
      ws.send(JSON.stringify({ id, method, params }))
    })
  }

  async function targetId() {
    if (pageTargetId) return pageTargetId
    return new Promise((resolveTarget, rejectTarget) => {
      const timer = setTimeout(() => rejectTarget(new Error('El inspector no anunció la página')), 5000)
      targetWaiters.push((id) => {
        clearTimeout(timer)
        resolveTarget(id)
      })
    })
  }

  return {
    ws,
    opened,
    async send(method, params = {}) {
      const id = ++innerId
      const target = await targetId()
      return new Promise((resolveResult, rejectResult) => {
        innerPending.set(id, { resolve: resolveResult, reject: rejectResult })
        sendOuter('Target.sendMessageToTarget', {
          targetId: target,
          message: JSON.stringify({ id, method, params }),
        }).catch((error) => {
          if (!innerPending.has(id)) return
          innerPending.delete(id)
          rejectResult(error)
        })
      })
    },
  }
}

async function withPage(run) {
  const page = await inspectorPage()
  const inspector = connectInspector(page.socketPath)
  await inspector.opened
  try {
    return await run(inspector, page)
  } finally {
    inspector.ws.close()
  }
}

async function evaluate(expression) {
  return withPage(async (inspector) => {
    const result = await inspector.send('Runtime.evaluate', {
      expression,
      returnByValue: true,
      awaitPromise: true,
    })
    if (result.exceptionDetails || result.wasThrown) {
      throw new Error(`La página rechazó la acción: ${JSON.stringify(result.exceptionDetails ?? result)}`)
    }
    return result.result?.value
  })
}

const FIND_CONTROLS = `
(role, name, withinHeading) => {
  const norm = (value) => (value || '').replace(/\\s+/g, ' ').trim()
  const labelOf = (element) => norm(element.getAttribute('aria-label') || element.textContent)
  const root = withinHeading
    ? [...document.querySelectorAll('h1,h2,h3')].find((heading) => norm(heading.textContent) === norm(withinHeading))?.closest('section')
    : document
  if (!root) return { ok: false, reason: 'heading-missing', heading: withinHeading }
  const selector = role === 'button'
    ? 'button, [role="button"]'
    : role === 'switch'
      ? 'button[role="switch"], [role="switch"]'
      : '[role="' + role + '"]'
  const nodes = [...root.querySelectorAll(selector)]
  const wanted = norm(name)
  const match = wanted ? nodes.find((element) => labelOf(element) === wanted) : nodes[0]
  if (!match) {
    return { ok: false, reason: 'control-missing', candidates: nodes.map(labelOf).filter(Boolean) }
  }
  return {
    ok: true,
    name: labelOf(match),
    ariaPressed: match.getAttribute('aria-pressed'),
    ariaChecked: match.getAttribute('aria-checked'),
  }
}
`

const SNAPSHOT_SCRIPT = `
() => {
  const norm = (value) => (value || '').replace(/\\s+/g, ' ').trim()
  const interesting = new Set(['BUTTON', 'A', 'INPUT', 'TEXTAREA', 'SELECT', 'H1', 'H2', 'H3', 'P'])
  const lines = []
  const walk = (element, depth) => {
    if (!(element instanceof Element) || depth > 12) return
    const explicit = element.getAttribute('role')
    const named = explicit || interesting.has(element.tagName)
    if (named) {
      const role = explicit || element.tagName.toLowerCase()
      const name = norm(element.getAttribute('aria-label') || element.textContent).slice(0, 120)
      const pressed = element.getAttribute('aria-pressed')
      const checked = element.getAttribute('aria-checked')
      const disabled = element.disabled || element.getAttribute('aria-disabled') === 'true'
      const flags = [
        pressed != null ? 'pressed=' + pressed : '',
        checked != null ? 'checked=' + checked : '',
        disabled ? 'disabled' : '',
      ].filter(Boolean).join(' ')
      lines.push('  '.repeat(depth) + role + ' "' + name + '"' + (flags ? ' ' + flags : ''))
    }
    for (const child of element.children) walk(child, named ? depth + 1 : depth)
  }
  walk(document.body, 0)
  return lines.join('\\n')
}
`

function parseArgs(argv) {
  const command = argv[0]
  const options = {}
  for (let index = 1; index < argv.length; index += 1) {
    const token = argv[index]
    if (!token.startsWith('--')) fail(`Argumento no reconocido: ${token}`)
    const key = token.slice(2)
    if (key === 'aria') {
      options.aria = true
      continue
    }
    const value = argv[index + 1]
    if (value == null || value.startsWith('--')) fail(`Falta el valor de --${key}`)
    options[key] = value
    index += 1
  }
  return { command, options }
}

async function inspectInstance() {
  const state = readState()
  const problems = []
  if (!pidAlive(state.pid)) problems.push(`El proceso ${state.pid} no está vivo.`)
  const tree = processTree(state.pid)
  const owner = listenerPid(inspectorPort)
  if (!owner || !tree.has(owner)) {
    problems.push(`El puerto ${inspectorPort} no pertenece a esta instancia.`)
  } else if (!envValue(owner, 'HOME').startsWith(state.home)) {
    problems.push(`HOME del inspector no es el directorio aislado ${state.home}.`)
  }
  let pageUrl = ''
  if (problems.length === 0) {
    try {
      const page = await inspectorPage()
      pageUrl = page.url
      if (!page) problems.push(`No hay una página en :${vitePort}.`)
      else {
        const text = await evaluate('document.body ? document.body.innerText : ""')
        if (!String(text).includes(readyText)) {
          problems.push(`La página todavía no muestra "${readyText}".`)
        }
      }
    } catch (error) {
      problems.push(error instanceof Error ? error.message : String(error))
    }
  }
  return {
    ok: problems.length === 0,
    pid: state.pid,
    home: state.home,
    dataDir: state.dataDir,
    url: pageUrl,
    problems,
  }
}

async function doctor() {
  const report = await inspectInstance()
  console.log(JSON.stringify(report, null, 2))
  if (!report.ok) process.exit(1)
}

async function launch() {
  if (existsSync(statePath)) {
    const existing = JSON.parse(readFileSync(statePath, 'utf8'))
    if (pidAlive(existing.pid)) {
      fail(`Ya hay una instancia de verificación (pid ${existing.pid}). Ejecuta cleanup antes de otro launch.`)
    }
  }
  if (await portOpen(vitePort)) fail(`El puerto ${vitePort} ya está ocupado. No se inicia otra instancia.`)
  if (await portOpen(inspectorPort)) fail(`El puerto ${inspectorPort} ya está ocupado. No se inicia otra instancia.`)

  const realHome = process.env.HOME
  if (!realHome) fail('HOME no está definido.')
  const home = `${root}/home`
  rmSync(home, { recursive: true, force: true })
  mkdirSync(`${home}/.local/share`, { recursive: true })
  mkdirSync(`${home}/.config`, { recursive: true })
  mkdirSync(`${home}/.cache`, { recursive: true })
  mkdirSync(evidenceRoot, { recursive: true })
  mkdirSync(root, { recursive: true })

  const logFd = openSync(logPath, 'w')
  const child = spawn('npm', ['run', 'tauri:dev'], {
    cwd: repoRoot,
    detached: true,
    stdio: ['ignore', logFd, logFd],
    env: {
      ...process.env,
      HOME: home,
      XDG_DATA_HOME: `${home}/.local/share`,
      XDG_CONFIG_HOME: `${home}/.config`,
      XDG_CACHE_HOME: `${home}/.cache`,
      CARGO_HOME: process.env.CARGO_HOME || `${realHome}/.cargo`,
      RUSTUP_HOME: process.env.RUSTUP_HOME || `${realHome}/.rustup`,
      npm_config_cache: process.env.npm_config_cache || `${realHome}/.npm`,
      WEBKIT_INSPECTOR_HTTP_SERVER: `${inspectorHost}:${inspectorPort}`,
      GDK_BACKEND: 'x11',
      WEBKIT_DISABLE_DMABUF_RENDERER: '1',
    },
  })
  closeSync(logFd)
  child.unref()
  if (!child.pid) fail('npm no entregó un pid.')

  const state = {
    pid: child.pid,
    home,
    dataDir: `${home}/.local/share/ro-launcher`,
    logPath,
    evidenceRoot,
    repoRoot,
  }
  writeFileSync(statePath, JSON.stringify(state, null, 2))

  const deadline = Date.now() + 240_000
  let lastProblem = 'aún no responde'
  while (Date.now() < deadline) {
    if (!pidAlive(child.pid)) {
      fail(`tauri dev terminó antes de estar listo. Revisa ${logPath}.`)
    }
    const report = await inspectInstance()
    if (report.ok) {
      console.log(JSON.stringify(report, null, 2))
      return
    }
    lastProblem = report.problems.join(' ')
    const logTail = existsSync(logPath) ? readFileSync(logPath, 'utf8').slice(-800) : ''
    if (logTail.toLowerCase().includes('address already in use')) {
      fail(`El puerto de desarrollo está ocupado. Revisa ${logPath}.`)
    }
    await delay(2000)
  }
  fail(`La instancia no estuvo lista en 240s (${lastProblem}). Revisa ${logPath}.`)
}

async function click(options) {
  if (!options.role) fail('click necesita --role.')
  if (!options.name && !options['within-heading']) {
    fail('click necesita --name o --within-heading.')
  }
  const args = `${JSON.stringify(options.role)}, ${JSON.stringify(options.name ?? '')}, ${JSON.stringify(options['within-heading'] ?? '')}`
  const before = await evaluate(`(${FIND_CONTROLS})(${args})`)
  if (!before?.ok) {
    console.log(JSON.stringify(before, null, 2))
    process.exit(1)
  }
  const role = JSON.stringify(options.role)
  const name = JSON.stringify(options.name ?? '')
  const heading = JSON.stringify(options['within-heading'] ?? '')
  await evaluate(`(() => {
    const norm = (value) => (value || '').replace(/\\s+/g, ' ').trim()
    const root = ${heading}
      ? [...document.querySelectorAll('h1,h2,h3')].find((node) => norm(node.textContent) === norm(${heading}))?.closest('section')
      : document
    const selector = ${role} === 'button'
      ? 'button, [role="button"]'
      : ${role} === 'switch'
        ? '[role="switch"]'
        : '[role="' + ${role} + '"]'
    const wanted = norm(${name})
    const nodes = [...root.querySelectorAll(selector)]
    const match = wanted
      ? nodes.find((element) => norm(element.getAttribute('aria-label') || element.textContent) === wanted)
      : nodes[0]
    match.click()
    return true
  })()`)
  await delay(150)
  const result = await evaluate(`(${FIND_CONTROLS})(${args})`)
  console.log(JSON.stringify(result, null, 2))
  if (!result?.ok) process.exit(1)
}

async function snapshot(options) {
  if (!options.path) fail('snapshot necesita --path.')
  const body = await evaluate(`(${SNAPSHOT_SCRIPT})()`)
  mkdirSync(dirname(options.path), { recursive: true })
  writeFileSync(options.path, `${body}\n`)
  console.log(options.path)
}

async function screenshot(options) {
  if (!options.path) fail('screenshot necesita --path.')
  const image = await withPage(async (inspector) => {
    const size = await inspector.send('Runtime.evaluate', {
      expression: '({w: window.innerWidth, h: window.innerHeight})',
      returnByValue: true,
    })
    const { w, h } = size.result?.value ?? {}
    if (!w || !h) throw new Error(`No se pudo leer el tamaño de la ventana: ${JSON.stringify(size)}`)
    const shot = await inspector.send('Page.snapshotRect', {
      x: 0,
      y: 0,
      width: w,
      height: h,
      coordinateSystem: 'Viewport',
    })
    const dataUrl = String(shot.dataURL ?? shot.data ?? '')
    const encoded = dataUrl.split(',')[1]
    if (!encoded) throw new Error('El inspector no devolvió una imagen')
    return Buffer.from(encoded, 'base64')
  })
  mkdirSync(dirname(options.path), { recursive: true })
  writeFileSync(options.path, image)
  console.log(options.path)
}

function cleanup() {
  if (!existsSync(statePath)) {
    console.log('No había instancia de verificación.')
    return
  }
  const state = JSON.parse(readFileSync(statePath, 'utf8'))
  const home = envValue(state.pid, 'HOME')
  if (pidAlive(state.pid) && home !== state.home) {
    fail(`No se cierra el pid ${state.pid}: su HOME es "${home}", no ${state.home}.`)
  }
  if (pidAlive(state.pid)) {
    try {
      process.kill(-state.pid, 'SIGTERM')
    } catch {
      try {
        process.kill(state.pid, 'SIGTERM')
      } catch {
        // The process already left.
      }
    }
  }
  const deadline = Date.now() + 8_000
  while (pidAlive(state.pid) && Date.now() < deadline) {
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 200)
  }
  if (pidAlive(state.pid)) {
    try {
      process.kill(-state.pid, 'SIGKILL')
    } catch {
      try {
        process.kill(state.pid, 'SIGKILL')
      } catch {
        // Already gone.
      }
    }
  }
  rmSync(state.home, { recursive: true, force: true })
  rmSync(statePath, { force: true })
  console.log(`Instancia cerrada. La evidencia sigue en ${state.evidenceRoot}.`)
}

const { command, options } = parseArgs(process.argv.slice(2))
try {
  if (command === 'launch') await launch()
  else if (command === 'doctor') await doctor()
  else if (command === 'click') await click(options)
  else if (command === 'snapshot') await snapshot(options)
  else if (command === 'screenshot') await screenshot(options)
  else if (command === 'cleanup') cleanup()
  else fail('Comando: launch | doctor | click | snapshot | screenshot | cleanup')
} catch (error) {
  fail(error instanceof Error ? error.message : String(error))
}
