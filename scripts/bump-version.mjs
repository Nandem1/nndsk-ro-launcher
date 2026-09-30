#!/usr/bin/env node

import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { spawnSync } from 'node:child_process'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const SEMVER = /^([0-9]+)\.([0-9]+)\.([0-9]+)$/

export function parseSemver(value) {
  const match = String(value).match(SEMVER)
  if (!match) return null
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    text: `${match[1]}.${match[2]}.${match[3]}`,
  }
}

export function nextVersion(current, spec) {
  const parsed = parseSemver(current)
  if (!parsed) {
    throw new Error(`current version is not X.Y.Z: ${current}`)
  }
  if (spec === 'patch') {
    return `${parsed.major}.${parsed.minor}.${parsed.patch + 1}`
  }
  if (spec === 'minor') {
    return `${parsed.major}.${parsed.minor + 1}.0`
  }
  if (spec === 'major') {
    return `${parsed.major + 1}.0.0`
  }
  const target = parseSemver(spec)
  if (!target) {
    throw new Error(`expected patch, minor, major, or X.Y.Z, got ${spec}`)
  }
  if (target.text === parsed.text) {
    throw new Error(`already at ${target.text}`)
  }
  return target.text
}

export function cargoPackageVersion(toml) {
  const match = toml.match(/^version\s*=\s*"([^"]+)"/m)
  if (!match) {
    throw new Error('no package version in src-tauri/Cargo.toml')
  }
  return match[1]
}

export function readAppVersion(files) {
  const versions = {
    'package.json': files.packageJson.version,
    'src-tauri/tauri.conf.json': files.tauriConf.version,
    'src-tauri/Cargo.toml': cargoPackageVersion(files.cargoToml),
  }
  const unique = new Set(Object.values(versions))
  if (unique.size !== 1) {
    throw new Error(`version drift: ${JSON.stringify(versions)}`)
  }
  return [...unique][0]
}

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'))
}

function loadRepoFiles() {
  return {
    packageJson: readJson(join(root, 'package.json')),
    packageLock: readFileSync(join(root, 'package-lock.json'), 'utf8'),
    tauriConf: readJson(join(root, 'src-tauri', 'tauri.conf.json')),
    cargoToml: readFileSync(join(root, 'src-tauri', 'Cargo.toml'), 'utf8'),
    cargoLock: readFileSync(join(root, 'Cargo.lock'), 'utf8'),
  }
}

function replaceOnce(haystack, needle, replacement, label) {
  const index = haystack.indexOf(needle)
  if (index === -1) {
    throw new Error(`missing ${label}`)
  }
  if (haystack.indexOf(needle, index + needle.length) !== -1) {
    throw new Error(`refusing to replace ${label}: more than one match`)
  }
  return (
    haystack.slice(0, index) + replacement + haystack.slice(index + needle.length)
  )
}

export function applyVersion(files, from, to) {
  const packageJson = { ...files.packageJson, version: to }
  const tauriConf = { ...files.tauriConf, version: to }
  const cargoToml = replaceOnce(
    files.cargoToml,
    `version = "${from}"`,
    `version = "${to}"`,
    'src-tauri/Cargo.toml package version',
  )
  const cargoLock = replaceOnce(
    files.cargoLock,
    `name = "ro-launcher"\nversion = "${from}"`,
    `name = "ro-launcher"\nversion = "${to}"`,
    'Cargo.lock ro-launcher version',
  )
  let packageLock = replaceOnce(
    files.packageLock,
    `"name": "ro-launcher",\n  "version": "${from}"`,
    `"name": "ro-launcher",\n  "version": "${to}"`,
    'package-lock.json root version',
  )
  packageLock = replaceOnce(
    packageLock,
    `"name": "ro-launcher",\n      "version": "${from}"`,
    `"name": "ro-launcher",\n      "version": "${to}"`,
    'package-lock.json workspace version',
  )
  return { packageJson, packageLock, tauriConf, cargoToml, cargoLock }
}

function writeRepoFiles(files) {
  writeFileSync(
    join(root, 'package.json'),
    `${JSON.stringify(files.packageJson, null, 2)}\n`,
  )
  writeFileSync(join(root, 'package-lock.json'), files.packageLock)
  writeFileSync(
    join(root, 'src-tauri', 'tauri.conf.json'),
    `${JSON.stringify(files.tauriConf, null, 2)}\n`,
  )
  writeFileSync(join(root, 'src-tauri', 'Cargo.toml'), files.cargoToml)
  writeFileSync(join(root, 'Cargo.lock'), files.cargoLock)
}

function printHelp() {
  console.log(`Usage:
  node scripts/bump-version.mjs --show
  node scripts/bump-version.mjs --dry-run <patch|minor|major|X.Y.Z>
  node scripts/bump-version.mjs <patch|minor|major|X.Y.Z>

Writes the same version to package.json, package-lock.json,
src-tauri/tauri.conf.json, src-tauri/Cargo.toml, and Cargo.lock.
Then runs scripts/check-release-invariants.mjs.
`)
}

function main(argv) {
  const args = argv.filter((arg) => arg !== '--')
  if (args.includes('-h') || args.includes('--help')) {
    printHelp()
    return
  }

  const files = loadRepoFiles()
  const current = readAppVersion(files)

  if (args[0] === '--show' || args.length === 0) {
    console.log(current)
    return
  }

  const dryRun = args.includes('--dry-run')
  const spec = args.find((arg) => arg !== '--dry-run')
  if (!spec) {
    throw new Error('missing patch, minor, major, or X.Y.Z')
  }

  const next = nextVersion(current, spec)
  const nextFiles = applyVersion(files, current, next)

  if (dryRun) {
    console.log(`bump-version: dry-run ${current} -> ${next}`)
    return
  }

  writeRepoFiles(nextFiles)
  const check = spawnSync(
    process.execPath,
    [join(root, 'scripts', 'check-release-invariants.mjs')],
    { cwd: root, stdio: 'inherit' },
  )
  if (check.status !== 0) {
    process.exit(check.status ?? 1)
  }
  console.log(`bump-version: ${current} -> ${next}`)
}

const invokedDirectly =
  Boolean(process.argv[1]) &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href

if (invokedDirectly) {
  try {
    main(process.argv.slice(2))
  } catch (error) {
    console.error(`bump-version: ${error.message}`)
    process.exit(1)
  }
}
