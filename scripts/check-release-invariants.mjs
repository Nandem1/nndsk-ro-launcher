#!/usr/bin/env node
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { sidecarOrderError } from './release-sidecar-order.mjs'
import { qualityCiTriggerError } from './quality-ci-trigger.mjs'
import {
  hiddenFailureErrors,
  releaseWorkflowErrors,
  unpinnedActionErrors,
} from './release-workflow-policy.mjs'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const EXPECTED_ENDPOINT =
  'https://github.com/Nandem1/nndsk-ro-launcher/releases/latest/download/latest.json'

function fail(message) {
  console.error(`check-release-invariants: ${message}`)
  process.exit(1)
}

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'))
}

function cargoPackageVersion(toml) {
  const match = toml.match(/^\s*version\s*=\s*"([^"]+)"/m)
  if (!match) fail(`no version in ${join('src-tauri', 'Cargo.toml')}`)
  return match[1]
}

function walkFiles(dir, out = []) {
  const skipDirs = new Set([
    'target',
    'binaries',
    'gen',
    'icons',
    'node_modules',
  ])
  for (const entry of readdirSync(dir)) {
    if (skipDirs.has(entry)) continue
    const path = join(dir, entry)
    const st = statSync(path)
    if (st.isDirectory()) walkFiles(path, out)
    else out.push(path)
  }
  return out
}

const args = process.argv.slice(2)
const tagIndex = args.indexOf('--tag')
const tag = tagIndex >= 0 ? args[tagIndex + 1] : null

const packageJson = readJson(join(root, 'package.json'))
const tauriConf = readJson(join(root, 'src-tauri', 'tauri.conf.json'))
const cargoToml = readFileSync(join(root, 'src-tauri', 'Cargo.toml'), 'utf8')
const cargoVersion = cargoPackageVersion(cargoToml)

const packageLock = readJson(join(root, 'package-lock.json'))
const cargoLock = readFileSync(join(root, 'Cargo.lock'), 'utf8')
const cargoLockLauncher = cargoLock.match(
  /\[\[package\]\]\nname = "ro-launcher"\nversion = "([^"]+)"/,
)

const versions = {
  'package.json': packageJson.version,
  'package-lock.json': packageLock.version,
  'package-lock.json packages[""]': packageLock.packages?.['']?.version,
  'src-tauri/tauri.conf.json': tauriConf.version,
  'src-tauri/Cargo.toml': cargoVersion,
  'Cargo.lock ro-launcher': cargoLockLauncher?.[1],
}

const unique = new Set(Object.values(versions))
if (unique.size !== 1) {
  fail(`version drift: ${JSON.stringify(versions)}`)
}

if (tag) {
  const expected = tag.startsWith('v') ? tag.slice(1) : tag
  if (expected !== packageJson.version) {
    fail(`tag ${tag} does not match version ${packageJson.version}`)
  }
}

if (tauriConf.bundle?.createUpdaterArtifacts !== true) {
  fail('bundle.createUpdaterArtifacts must be true')
}

const updater = tauriConf.plugins?.updater
if (!updater) fail('plugins.updater missing')
if (updater.dangerousInsecureTransportProtocol === true) {
  fail('dangerousInsecureTransportProtocol must not be true')
}
if (updater.dangerousAcceptInvalidCerts === true) {
  fail('dangerousAcceptInvalidCerts must not be true')
}
if (updater.dangerousAcceptInvalidHostnames === true) {
  fail('dangerousAcceptInvalidHostnames must not be true')
}
if (updater.requireSignedVersion !== true) {
  fail('plugins.updater.requireSignedVersion must be true')
}
if (updater.allowDowngrades === true) {
  fail('plugins.updater.allowDowngrades must not be true')
}

const endpoints = updater.endpoints
if (
  !Array.isArray(endpoints) ||
  endpoints.length !== 1 ||
  endpoints[0] !== EXPECTED_ENDPOINT
) {
  fail(
    `plugins.updater.endpoints must be exactly [${JSON.stringify(EXPECTED_ENDPOINT)}]`,
  )
}

const pubkey = updater.pubkey
if (typeof pubkey !== 'string' || pubkey.trim() === '') {
  fail('plugins.updater.pubkey must be non-empty contents')
}
if (
  pubkey.includes('/') ||
  pubkey.endsWith('.pem') ||
  pubkey.endsWith('.key')
) {
  fail('plugins.updater.pubkey looks like a path; paste key contents')
}

const deps = {
  ...(packageJson.dependencies ?? {}),
  ...(packageJson.devDependencies ?? {}),
}
for (const name of [
  '@tauri-apps/plugin-updater',
  '@tauri-apps/plugin-process',
]) {
  if (name in deps) fail(`${name} must not be in package.json`)
}

const cliRange = packageJson.devDependencies?.['@tauri-apps/cli']
if (typeof cliRange !== 'string' || !/^\^2\.(1[2-9]|[2-9]\d)/.test(cliRange)) {
  fail(
    '@tauri-apps/cli must be ^2.12.0 or later so signatures include version:',
  )
}

const capabilityFiles = walkFiles(join(root, 'src-tauri', 'capabilities'))
for (const file of capabilityFiles) {
  const text = readFileSync(file, 'utf8')
  if (text.includes('updater:default') || text.includes('updater:allow-')) {
    fail(`${file} must not grant updater:default or updater:allow-*`)
  }
}

const textScan = /\.(rs|ts|tsx|js|mjs|json|yml|yaml|md|toml)$/
const secretNeedles = [
  ['minisign', 'encrypted', 'secret', 'key'].join(' '),
  ['MINISIGN', 'PRIVATE', 'KEY'].join(' '),
  ['BEGIN', 'OPENSSH', 'PRIVATE', 'KEY'].join(' '),
  ['TAURI_SIGNING_PRIVATE_KEY', '='].join(''),
]
const scanRoots = ['src', 'src-tauri', 'scripts', '.github', 'crates', 'docs']
for (const rel of scanRoots) {
  const dir = join(root, rel)
  try {
    statSync(dir)
  } catch {
    continue
  }
  for (const file of walkFiles(dir)) {
    if (file.endsWith('.key') && !file.endsWith('.pub')) {
      fail(`private key file must not be in the tree: ${file}`)
    }
    if (!textScan.test(file)) continue
    const text = readFileSync(file, 'utf8')
    for (const needle of secretNeedles) {
      if (text.includes(needle)) {
        fail(`secret material marker ${JSON.stringify(needle)} in ${file}`)
      }
    }
  }
}

const releaseYaml = readFileSync(
  join(root, '.github', 'workflows', 'release.yml'),
  'utf8',
)
const sidecarError = sidecarOrderError(releaseYaml)
if (sidecarError) fail(sidecarError)
const releaseErrors = releaseWorkflowErrors(releaseYaml)
if (releaseErrors.length > 0) fail(releaseErrors.join('; '))

const ciYaml = readFileSync(
  join(root, '.github', 'workflows', 'ci.yml'),
  'utf8',
)
const ciTriggerError = qualityCiTriggerError(ciYaml)
if (ciTriggerError) fail(ciTriggerError)

const workflowFiles = {}
for (const file of walkFiles(join(root, '.github'))) {
  if (/\.ya?ml$/.test(file)) workflowFiles[file] = readFileSync(file, 'utf8')
}
const workflowErrors = [
  ...unpinnedActionErrors(workflowFiles),
  ...hiddenFailureErrors(workflowFiles),
]
if (workflowErrors.length > 0) fail(workflowErrors.join('; '))

const mainWindow = tauriConf.app?.windows?.[0]
if (!mainWindow || mainWindow.decorations !== false) {
  fail(
    'app.windows[0].decorations must be false so GTK CSD does not duplicate the in-app title',
  )
}
const capabilities = readFileSync(
  join(root, 'src-tauri', 'capabilities', 'default.json'),
  'utf8',
)
for (const perm of [
  'core:window:allow-close',
  'core:window:allow-minimize',
  'core:window:allow-start-dragging',
]) {
  if (!capabilities.includes(perm)) {
    fail(`undecorated window requires ${perm} in capabilities/default.json`)
  }
}

const webviewRs = readFileSync(
  join(root, 'src-tauri', 'src', 'utils', 'webview.rs'),
  'utf8',
)
if (!webviewRs.includes('set_titlebar(None')) {
  fail(
    'webview.rs must clear the GTK titlebar; tao Wayland ignores decorations:false',
  )
}

console.log(
  `check-release-invariants: ok (version ${packageJson.version}${tag ? `, tag ${tag}` : ''})`,
)
