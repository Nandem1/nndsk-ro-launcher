// Explicit network integration smoke. Never run by normal CI; no games or user prefixes.
import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { openSync, closeSync } from 'node:fs'
import { mkdtemp, mkdir, readFile, realpath, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import readline from 'node:readline'

const options = Object.fromEntries(process.argv.slice(2).map((arg) => {
  const split = arg.indexOf('=')
  if (!arg.startsWith('--') || split < 0) throw new Error('Use --key=value')
  return [arg.slice(2, split), arg.slice(split + 1)]
}))
for (const key of ['umu', 'proton', 'sessiond', 'output']) {
  if (!options[key]) throw new Error(`Missing --${key}=absolute/path`)
  options[key] = await realpath(options[key])
}
const root = options['resume-root']
  ? await realpath(options['resume-root'])
  : await mkdtemp(path.join(tmpdir(), 'ro-first-run-smoke-'))
const home = path.join(root, 'home')
const marker = path.join(root, 'ro-first-run-smoke.json')
if (options['resume-root']) {
  const receipt = JSON.parse(await readFile(marker, 'utf8'))
  if (receipt.protocol !== 1 || receipt.root !== root || receipt.proton !== options.proton) {
    throw new Error('Retry only allowed on a HOME owned by this harness and the same Proton')
  }
} else {
  await mkdir(home)
  await writeFile(marker, JSON.stringify({ protocol: 1, root, proton: options.proton }), { flag: 'wx', mode: 0o600 })
  if (options['seed-partial'] === '1') {
    // Incomplete previous download/extraction: no successful-install marker, no user state.
    await mkdir(path.join(home, '.local/share/umu/steamrt4/pressure-vessel'), { recursive: true })
    await writeFile(path.join(home, '.local/share/umu/steamrt4/VERSIONS.txt'), 'incomplete fixture\n')
    await mkdir(path.join(home, '.cache/umu'), { recursive: true })
    await writeFile(path.join(home, '.cache/umu/unrelated.partial'), 'incomplete fixture\n')
  }
}
const prefix = path.join(home, '.local/share/ro-launcher/prefixes/smoke')
const env = {
  PATH: '/usr/local/bin:/usr/bin:/bin',
  HOME: home,
  XDG_DATA_HOME: path.join(home, '.local/share'),
  XDG_CONFIG_HOME: path.join(home, '.config'),
  XDG_CACHE_HOME: path.join(home, '.cache'),
  TMPDIR: root,
  LANG: 'C.UTF-8',
}
// Keep only the real desktop sockets, not the workstation's app/runtime/cache state.
for (const key of ['DISPLAY', 'XAUTHORITY', 'XDG_RUNTIME_DIR', 'DBUS_SESSION_BUS_ADDRESS']) {
  if (process.env[key]) env[key] = process.env[key]
}
const runnerEnv = {
  WINEPREFIX: prefix, PROTONPATH: path.dirname(options.proton), GAMEID: '0',
  PROTON_VERB: 'waitforexitandrun',
  UMU_RUNTIME_UPDATE: '0', UMU_LOG: 'debug',
  WAYLAND_DISPLAY: '',
}
const ca = '/etc/ssl/certs/ca-certificates.crt'
if (await stat(ca).then((s) => s.isFile(), () => false)) {
  runnerEnv.CURL_CA_BUNDLE = ca
  runnerEnv.SSL_CERT_FILE = ca
}
const hash = async (filename) => createHash('sha256').update(await readFile(filename)).digest('hex')
const metadata = { root, home, prefix, env, runnerEnv, binaries: {}, startedAt: new Date().toISOString() }
for (const key of ['umu', 'proton', 'sessiond']) {
  metadata.binaries[key] = { path: options[key], sha256: await hash(options[key]) }
}
const log = openSync(path.join(options.output, 'supervisor-stderr.log'), 'wx', 0o600)
const events = []
const child = spawn(options.sessiond, ['--prefix', prefix, '--parent-pid', `${process.pid}`], {
  env, stdio: ['pipe', 'pipe', log],
})
child.stdin.on('error', () => {}) // A failed handshake may close stdin before cleanup.
const request = (message) => child.stdin.write(`${JSON.stringify(message)}\n`)
request({ type: 'hello', protocolVersion: 1 })
let result
let shutdown = false
const timer = setTimeout(() => {
  result = { stage: 'timeout', seconds: Number(options.timeout ?? 600) }
  stop()
}, Number(options.timeout ?? 600) * 1000)
let cleanupTimer
function stop() {
  if (shutdown) return
  shutdown = true
  cleanupTimer = setTimeout(() => {
    // Only our own child; parent-death cleanup is the supervisor's fallback contract.
    child.kill('SIGTERM')
  }, 15_000)
  request({ type: 'shutdown', requestId: 'cleanup', graceMs: 1000, shutdownSpec: {
    program: '/usr/bin/true', args: [], cwd: home,
    env: [{ key: 'WINEPREFIX', value: prefix }],
  } })
}
const lines = readline.createInterface({ input: child.stdout })
for await (const line of lines) {
  const event = JSON.parse(line)
  events.push({ elapsedMs: Date.now() - Date.parse(metadata.startedAt), ...event })
  console.log(JSON.stringify(event))
  if (event.type === 'ready') {
    // Actual prefix setup creates its owned root only after the supervisor handshake.
    await mkdir(prefix, { recursive: true })
    request({ type: 'launch', requestId: 'initialize', spec: {
      program: options.umu, args: options.target === 'empty' ? [''] : ['wineboot', '-i'], cwd: prefix,
      env: Object.entries(runnerEnv).map(([key, value]) => ({ key, value })),
    } })
  }
  if (event.type === 'controllerExited' && event.requestId === 'initialize') {
    result = event
    stop()
  }
  if (event.type === 'error') {
    result = event
    stop()
  }
}
clearTimeout(timer)
clearTimeout(cleanupTimer)
const exit = await new Promise((resolve) => {
  if (child.exitCode !== null) resolve(child.exitCode)
  else child.once('exit', resolve)
})
closeSync(log)
const checks = {}
for (const relative of ['dosdevices/c:', 'drive_c/windows', 'system.reg', 'user.reg']) {
  checks[relative] = await stat(path.join(prefix, relative)).then(() => true, () => false)
}
await writeFile(path.join(options.output, 'result.json'), JSON.stringify({ ...metadata, events, result, checks, supervisorExit: exit }, null, 2))
console.log(`Evidence: ${options.output}; isolated HOME preserved: ${home}`)
process.exitCode = result?.exitCode === 0 && Object.values(checks).every(Boolean) && exit === 0 ? 0 : 1
