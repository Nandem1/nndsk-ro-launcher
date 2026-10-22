#!/usr/bin/env node
import { readFileSync, readdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { workflowJobs } from './release-workflow-policy.mjs'

const RUST_INSTALL =
  'rustup self update && rustup toolchain install --no-self-update'
const REQUIRED_JOBS = {
  '.github/workflows/ci.yml': ['quality', 'release-cache'],
  '.github/workflows/release.yml': [
    'release-config',
    'build',
    'sign',
    'publish',
  ],
  '.github/workflows/dependency-audit.yml': ['npm', 'rustsec'],
}
const RUST_JOBS = new Set([
  'ci.yml:quality',
  'ci.yml:release-cache',
  'release.yml:build',
  'dependency-audit.yml:rustsec',
])
const NODE_JOBS = new Set([
  ...RUST_JOBS,
  'release.yml:sign',
  'release.yml:publish',
  'dependency-audit.yml:npm',
])

// Deliberately accept only the small, literal pin schema we support. Unknown TOML/YAML
// structures must fail, rather than being partially parsed as a successful policy check.
export function rustToolchainVersion(text) {
  const match = text
    ?.trim()
    .match(
      /^\[toolchain\]\s*\nchannel = "(1\.\d+\.\d+)"\s*\ncomponents = \["rustfmt", "clippy"\]\s*\nprofile = "minimal"$/,
    )
  if (!match)
    throw new Error(
      'rust-toolchain.toml must declare an exact channel, rustfmt/clippy and profile = "minimal" using the supported literal schema',
    )
  return match[1]
}

function stepsOf(text, label) {
  if ([...text.matchAll(/^ {4}steps:\s*$/gm)].length !== 1)
    throw new Error(`${label}: expected exactly one literal steps list`)
  const starts = [...text.matchAll(/^ {6}- /gm)].map((match) => match.index)
  if (starts.length === 0)
    throw new Error(`${label}: cannot understand the steps list`)
  return starts.map((start, index) =>
    text
      .slice(start, starts[index + 1] ?? text.length)
      .replace(/^\s*#.*$/gm, '')
      .trimEnd(),
  )
}

function jobErrors(path, name, text) {
  const label = `${path}:${name}`
  const key = `${path.split('/').at(-1)}:${name}`
  const errors = []
  const steps = stepsOf(text, label)
  const nodeSetups = steps
    .map((step, index) =>
      /uses: actions\/setup-node@/.test(step) ? index : -1,
    )
    .filter((index) => index >= 0)
  const rustSetups = steps
    .map((step, index) => (step.includes(RUST_INSTALL) ? index : -1))
    .filter((index) => index >= 0)
  const needsRust =
    RUST_JOBS.has(key) ||
    /\b(cargo|rustc|rustfmt|rustup)\b|Swatinem\/rust-cache/.test(text)
  const needsNode =
    NODE_JOBS.has(key) ||
    /\b(node|npm|npx|corepack)\b|actions\/setup-node/.test(text)

  if (needsNode && nodeSetups.length !== 1)
    errors.push(`${label}: must set up Node exactly once from .nvmrc`)
  for (const index of nodeSetups) {
    const step = steps[index]
    if (
      !/^ {6}- uses: actions\/setup-node@[a-f0-9]{40}(?: #[^\n]*)?\n {8}with:\n {10}node-version-file: \.nvmrc(?:\n {10}cache: npm)?$/.test(
        step,
      )
    )
      errors.push(
        `${label}: setup-node must consume node-version-file: .nvmrc only`,
      )
    if (/^ {8}(if|env|working-directory):/m.test(step))
      errors.push(
        `${label}: Node setup must be unconditional without environment/directory overrides`,
      )
    if (
      steps
        .slice(0, index)
        .some((before) => /\b(node|npm|npx|corepack)\b/.test(before))
    )
      errors.push(`${label}: Node setup must precede repository Node commands`)
  }
  if (needsRust && rustSetups.length !== 1)
    errors.push(`${label}: must install Rust exactly once with ${RUST_INSTALL}`)
  for (const index of rustSetups) {
    if (
      !/^ {6}- name: Install repository Rust toolchain\n {8}run: rustup self update && rustup toolchain install --no-self-update$/.test(
        steps[index],
      )
    )
      errors.push(
        `${label}: Rust installation must unconditionally consume rust-toolchain.toml without overrides`,
      )
    if (
      steps
        .slice(0, index)
        .some((before) =>
          /\b(cargo|rustc|rustfmt|rustup)\b|Swatinem\/rust-cache/.test(before),
        )
    )
      errors.push(
        `${label}: Rust installation must precede Rust commands and cache restoration`,
      )
  }
  if (
    path.endsWith('/release.yml') &&
    ['sign', 'publish'].includes(name) &&
    needsRust
  )
    errors.push(`${label}: must not install or invoke Rust`)
  return errors
}

/** Pure policy over a complete file inventory, also used by mutation tests. */
export function toolchainInvariantErrors(files) {
  const errors = []
  try {
    rustToolchainVersion(files['rust-toolchain.toml'])
  } catch (error) {
    errors.push(error.message)
  }
  if (!/^\d+\.\d+\.\d+\n$/.test(files['.nvmrc'] ?? ''))
    errors.push(
      '.nvmrc must contain one exact Node X.Y.Z release and a newline',
    )
  try {
    const pkg = JSON.parse(files['package.json'])
    if (pkg.engines?.node !== '>=22')
      errors.push(
        'package.json engines.node must retain the >=22 compatibility contract',
      )
    if (Number(files['.nvmrc']?.split('.')[0]) < 22)
      errors.push('.nvmrc must satisfy engines.node >=22')
    for (const [name, command] of Object.entries(pkg.scripts ?? {})) {
      if (
        /cargo\s+\+|rustup\s+(default|override)|RUSTUP_TOOLCHAIN\s*=|npm\s+(install|i)\s+.*(-g|--global)\b|npx\s+(?!--no-install\s)/.test(
          command,
        )
      )
        errors.push(
          `package.json scripts.${name}: bypasses repository toolchains or lockfile tools`,
        )
    }
  } catch {
    errors.push('package.json must be present and valid JSON')
  }

  for (const [path, names] of Object.entries(REQUIRED_JOBS)) {
    if (!(path in files)) {
      errors.push(`${path}: required workflow missing from inventory`)
      continue
    }
    const jobs = workflowJobs(files[path])
    for (const name of names)
      if (!(name in jobs))
        errors.push(`${path}: required job ${name} missing or unrecognized`)
  }
  for (const [path, source] of Object.entries(files)) {
    if (!/^\.github\/(workflows|actions)\/.+\.ya?ml$/.test(path)) continue
    const text = source.replace(/^\s*#.*$/gm, '')
    if (/\t|^\s*(?:<<:|[\w-]+:\s*[&*])|^\s*working-directory:/m.test(text))
      errors.push(
        `${path}: unsupported YAML aliases, tabs or working-directory override`,
      )
    if (
      /\b(?:RUSTUP_TOOLCHAIN|RUSTC|RUSTDOC|RUSTFMT)\s*[:=]|cargo\s+\+|\brustup\b|toolchain:|uses:.*(?:rust-toolchain|setup-rust)/.test(
        text.replaceAll(RUST_INSTALL, ''),
      )
    )
      errors.push(
        `${path}: independent Rust selector; use rust-toolchain.toml via ${RUST_INSTALL}`,
      )
    if (
      /node-version:|npm\s+(?:install|i)\s+.*(?:-g|--global)\b|corepack\s/.test(
        text,
      )
    )
      errors.push(
        `${path}: independent Node/npm selector; use setup-node with .nvmrc and its bundled npm`,
      )
    if (/\bnpx\s+(?!--no-install\s)/.test(text))
      errors.push(
        `${path}: npx must use --no-install so tools cannot escape package-lock.json`,
      )

    if (path.startsWith('.github/actions/')) {
      if (
        /\b(cargo|rustc|rustfmt|rustup|node|npm|npx)\b|setup-node|rust-toolchain|using:\s*node/.test(
          text,
        )
      )
        errors.push(
          `${path}: language tooling in a local action is unsupported; keep toolchain setup in checked workflow jobs`,
        )
      continue
    }
    const jobs = workflowJobs(text)
    const headers =
      text.slice(text.indexOf('jobs:')).match(/^ {2}[\w-]+:\s*$/gm) ?? []
    const unknownHeaders = text
      .slice(text.indexOf('jobs:'))
      .split('\n')
      .some((line) => /^ {2}\S/.test(line) && !/^ {2}[\w-]+:\s*$/.test(line))
    if (
      Object.keys(jobs).length === 0 ||
      headers.length !== Object.keys(jobs).length ||
      unknownHeaders
    )
      errors.push(`${path}: missing, duplicate or unrecognized job structure`)
    for (const [name, job] of Object.entries(jobs)) {
      try {
        errors.push(...jobErrors(path, name, job))
      } catch (error) {
        errors.push(error.message)
      }
    }
  }
  return errors
}

export function checkToolchainInvariants(root) {
  const files = {}
  for (const path of ['rust-toolchain.toml', '.nvmrc', 'package.json']) {
    try {
      files[path] = readFileSync(join(root, path), 'utf8')
    } catch (error) {
      throw new Error(`${path}: cannot read required authority (${error.code})`)
    }
  }
  function walk(path) {
    for (const entry of readdirSync(join(root, path), {
      withFileTypes: true,
    })) {
      const relative = `${path}/${entry.name}`
      if (entry.isDirectory()) walk(relative)
      else if (/\.ya?ml$/.test(relative))
        files[relative] = readFileSync(join(root, relative), 'utf8')
    }
  }
  walk('.github/workflows')
  walk('.github/actions')
  return toolchainInvariantErrors(files)
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    const errors = checkToolchainInvariants(
      join(dirname(fileURLToPath(import.meta.url)), '..'),
    )
    if (errors.length) throw new Error(errors.join('\n'))
    console.log('check-toolchain-invariants: ok')
  } catch (error) {
    console.error(`check-toolchain-invariants: ${error.message}`)
    process.exitCode = 1
  }
}
