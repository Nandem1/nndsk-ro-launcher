// Production UI integration: missing requirements -> guidance -> recheck -> setup.
// Synthetic IPC only; never launches a real runner or touches user prefixes.
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { chromium } from 'playwright'
import { auditFit } from './design-fit.mjs'

const port = process.argv[2] ?? '5175'
const output = process.env.RO_ENVIRONMENT_REVIEW_OUTPUT ?? '/tmp/ro-environment-review'
fs.mkdirSync(output, { recursive: true })
const csp = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf8')).app.security.csp
const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true })
const results = []
try {
  for (const viewport of [{ width: 1440, height: 900 }, { width: 1280, height: 820 }]) {
    const page = await browser.newPage({ viewport })
    const errors = []
    page.on('pageerror', (error) => errors.push(error.message))
    await page.route(`http://127.0.0.1:${port}/?*`, async (route) => {
      const response = await route.fetch()
      await route.fulfill({ response, headers: { ...response.headers(), 'content-security-policy': csp } })
    })
    await page.addInitScript(() => {
      const runner = { id: 'wine-7.16-staging-tkg-amd64', name: 'Wine 7.16 Staging/TkG amd64', path: '/fixture/bin/wine' }
      const server = { id: 'environment-fixture', name: 'SakuraRO', executablePath: '/fixture/ragexe.exe', prefixMode: 'isolated', runner: runner.path }
      const tool = { found: false, path: null, label: null }
      const environment = {
        schemaVersion: 1, provider: 'wineHostTools', launch: 'blocked', preparationAvailable: false, rebuildAvailable: false,
        issues: ['cabextract', 'unzip', 'downloader', 'winetricks'].map((tool) => ({
          code: `host-${tool}`, domain: 'tools', severity: 'error', affects: ['prepare', 'rebuild'],
          title: `Herramienta ${tool}`, message: `Falta ${tool} para instalar los componentes de Wine`,
          remediation: `Instala ${tool} con el administrador de paquetes del sistema y vuelve a comprobar el entorno`,
        })),
        actions: [{ kind: 'recheck' }, { kind: 'hostGuide', family: 'arch', packages: ['cabextract', 'curl', 'unzip', 'winetricks'] }],
      }
      const deps = {
        wine: true, winetricks: false, dxvk: false, prefixConfigured: false,
        audioOk: true, audioDriver: 'pulse', audioStack: 'pulse', audioWarning: null,
        inputGroupOk: true, inputGroupWarning: null, uinputInputOk: true, uinputInputWarning: null,
        prefixOk: false, prefixWarning: 'El entorno aislado se construirá antes de jugar',
        dxvkOk: true, dxvkWarning: null, runnerKind: 'wine', runnerOk: true, runnerWarning: null,
        prefixPath: '/fixture/prefix', prefixScope: 'isolated', prefixManaged: true,
        readyToLaunch: false, canSetup: false, canReset: false, checks: [], environment,
      }
      const responses = {
        list_servers: [server], load_settings: { defaultRunner: runner.path, richPresenceEnabled: false }, list_runners: [runner],
        list_game_clients: [], list_client_profiles: [], take_storage_notices: [], list_runtime_observations: [], list_runtime_benchmarks: [],
        scan_server_tools: {
          gameDir: '/fixture', openSetup: tool, patcher: tool,
          dgvoodoo: { cpl: tool, d3dimmDll: tool, ddrawDll: tool, conf: tool, configured: false, needsInstall: false, canAutoInstall: false, canUninstall: false, issues: [] },
          diagnostics: { architecture: 'x86', graphicsApis: ['d3d9'], managedPatcher: false, webview2Required: false, peAnalysisConclusive: true, gepardPresent: false, gameguardPresent: false, warnings: [] },
        },
        get_update_snapshot: { currentVersion: '0.2.2', phase: { kind: 'unavailable', reason: 'notPackaged' } },
      }
      let callbackId = 0
      window.__environmentFixture = {
        setups: 0, launches: 0,
        resolveTools() {
          deps.canSetup = true
          deps.winetricks = true
          deps.environment = { ...environment, launch: 'needsPreparation', preparationAvailable: true, issues: [], actions: [{ kind: 'recheck' }, { kind: 'prepare' }] }
        },
      }
      window.__TAURI_INTERNALS__ = {
        transformCallback: () => ++callbackId, unregisterCallback: () => {},
        metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
        invoke: async (command) => {
          if (command === 'check_dependencies') return structuredClone(deps)
          if (command === 'setup_prefix') {
            window.__environmentFixture.setups++
            Object.assign(deps, { readyToLaunch: true, prefixOk: true, dxvk: true, prefixWarning: null })
            deps.environment = { ...deps.environment, launch: 'ready', actions: [{ kind: 'recheck' }] }
            return null
          }
          if (command === 'launch_game') { window.__environmentFixture.launches++; throw new Error('Unexpected game launch') }
          if (command === 'plugin:event|listen') return ++callbackId
          if (command === 'plugin:app|version') return '0.2.2'
          if (command in responses) return structuredClone(responses[command])
          if (/update/.test(command)) return responses.get_update_snapshot
          return null
        },
      }
      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }
    })
    await page.goto(`http://127.0.0.1:${port}/?fixture=environment`)
    await page.getByRole('button', { name: 'Resolver requisitos' }).click()
    const dialog = page.getByRole('dialog', { name: 'Diagnóstico de SakuraRO' })
    await dialog.waitFor()
    await page.evaluate(() => document.fonts.ready)
    for (const tool of ['cabextract', 'unzip', 'downloader', 'winetricks']) {
      assert.equal(await dialog.getByText(`Herramienta ${tool}`, { exact: true }).count(), 1)
    }
    const fit = await auditFit(page)
    assert.deepEqual(fit.failures, [])
    await page.screenshot({ path: path.join(output, `requirements-${viewport.width}.png`) })
    // Expanding every technical detail must remain reachable in the single body scroll.
    await dialog.locator('details').evaluateAll((details) => details.forEach((detail) => { detail.open = true }))
    assert.deepEqual((await auditFit(page)).failures, [])
    await page.screenshot({ path: path.join(output, `details-${viewport.width}.png`) })
    await page.evaluate(() => window.__environmentFixture.resolveTools())
    await dialog.getByRole('button', { name: 'Volver a comprobar' }).click()
    await dialog.getByText('Listo para preparar', { exact: true }).waitFor()
    await dialog.getByRole('button', { name: 'Preparar entorno' }).click()
    await dialog.getByText('Entorno listo', { exact: true }).waitFor()
    assert.deepEqual(await page.evaluate(() => ({ setups: window.__environmentFixture.setups, launches: window.__environmentFixture.launches })), { setups: 1, launches: 0 })
    await page.screenshot({ path: path.join(output, `ready-${viewport.width}.png`) })
    await dialog.getByRole('button', { name: 'Cerrar' }).click()
    await page.getByRole('button', { name: 'Jugar', exact: true }).waitFor()
    assert.deepEqual(errors, [])
    results.push({ viewport, fit, outcome: 'passed' })
    await page.close()
  }
  fs.writeFileSync(path.join(output, 'result.json'), JSON.stringify(results, null, 2))
  console.log(`Environment UI passed at both sizes. Evidence: ${output}`)
} finally {
  await browser.close()
}
