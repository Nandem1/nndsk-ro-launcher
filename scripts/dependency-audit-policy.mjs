#!/usr/bin/env node
// Dependency audit policy shared by dependency-audit.yml and release.yml.
//
// The workflow must never turn green because an audit command crashed or because its exit code
// was swallowed. This script runs the audit itself, refuses any report it cannot parse, and then
// applies an explicit policy:
//
// - npm runtime graph (`--omit=dev`, conservatively the frontend dependencies used by the
//   bundled WebView assets, not node_modules shipped as a Node runtime): moderate or worse
//   blocks; low is reported.
// - npm dev/build toolchain only: critical blocks; high/moderate/low are reported as warnings so
//   they stay visible without making every toolchain ReDoS a release blocker.
// - RustSec: every vulnerability blocks (Cargo.lock is the shipped binary plus its build graph).
//   Informational warnings (unmaintained, unsound, yanked) are reported, never hidden.
import { spawnSync } from 'node:child_process'
import { appendFileSync, lstatSync } from 'node:fs'
import { homedir } from 'node:os'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { stripVTControlCharacters } from 'node:util'

const SEVERITY_RANK = { info: 0, low: 1, moderate: 2, high: 3, critical: 4 }

function rank(severity) {
  if (!Object.hasOwn(SEVERITY_RANK, severity))
    throw new Error(`unknown npm severity ${JSON.stringify(severity)}`)
  return SEVERITY_RANK[severity]
}

const isRecord = (value) =>
  value !== null && typeof value === 'object' && !Array.isArray(value)
const isText = (value) => typeof value === 'string' && value.trim() !== ''
const isCount = (value) => Number.isSafeInteger(value) && value >= 0

export function parseAuditJson(label, stdout) {
  let report
  try {
    report = JSON.parse(stdout)
  } catch (error) {
    throw new Error(`${label}: audit output is not JSON (${error.message})`)
  }
  if (report && typeof report === 'object' && 'error' in report) {
    throw new Error(
      `${label}: audit reported an error: ${JSON.stringify(report.error)}`,
    )
  }
  return report
}

function npmAdvisories(report, label) {
  if (
    report?.auditReportVersion !== 2 ||
    !isRecord(report.vulnerabilities) ||
    !isRecord(report.metadata?.vulnerabilities)
  ) {
    throw new Error(
      `${label}: npm audit report has no vulnerabilities/metadata section`,
    )
  }
  const counts = report.metadata.vulnerabilities
  const actual = Object.fromEntries(
    Object.keys(SEVERITY_RANK).map((s) => [s, 0]),
  )
  const entries = report.vulnerabilities
  const advisories = new Map()
  for (const [pkg, entry] of Object.entries(entries)) {
    if (
      !isRecord(entry) ||
      entry.name !== pkg ||
      !Array.isArray(entry.via) ||
      !entry.via.length
    )
      throw new Error(`${label}: malformed npm vulnerability ${pkg}`)
    rank(entry.severity)
    actual[entry.severity]++
    for (const via of entry.via) {
      if (typeof via === 'string') {
        if (!Object.hasOwn(entries, via))
          throw new Error(`${label}: missing meta-vulnerability ${via}`)
      } else {
        if (
          !isRecord(via) ||
          !Number.isSafeInteger(via.source) ||
          via.source <= 0 ||
          !isText(via.url) ||
          !isText(via.title)
        )
          throw new Error(`${label}: malformed advisory in ${pkg}`)
        rank(via.severity)
      }
    }
  }
  // npm 10 counts vulnerable packages (including meta-vulnerabilities), not unique advisories.
  for (const severity of Object.keys(counts)) {
    if (severity !== 'total') rank(severity)
  }
  if (
    !isCount(counts.total) ||
    counts.total !== Object.keys(entries).length ||
    Object.keys(actual).some(
      (s) => !isCount(counts[s]) || counts[s] !== actual[s],
    )
  )
    throw new Error(`${label}: inconsistent npm vulnerability counts`)

  function collect(pkg, seen = new Set()) {
    if (seen.has(pkg)) return []
    const next = new Set([...seen, pkg])
    return entries[pkg].via.flatMap((via) =>
      typeof via === 'string' ? collect(via, next) : [via],
    )
  }
  for (const [pkg, entry] of Object.entries(entries)) {
    const sources = collect(pkg)
    if (
      !sources.length ||
      rank(entry.severity) >
        Math.max(...sources.map((via) => rank(via.severity)))
    )
      throw new Error(`${label}: unresolved meta-vulnerability ${pkg}`)
    for (const via of sources) {
      // npm's numeric source survives package renames and meta-vulnerability paths.
      const known = advisories.get(via.source)
      if (known) {
        known.packages.add(pkg)
        if (rank(via.severity) > rank(known.severity))
          known.severity = via.severity
      } else {
        advisories.set(via.source, {
          id: via.url,
          title: via.title,
          severity: via.severity,
          packages: new Set([pkg]),
        })
      }
    }
  }
  return advisories
}

/** Classifies npm advisories. `full` is `npm audit --json`, `runtime` adds `--omit=dev`. */
export function classifyNpmAudit(full, runtime) {
  const all = npmAdvisories(full, 'npm audit')
  const shipped = npmAdvisories(runtime, 'npm audit --omit=dev')
  const blocking = []
  const warnings = []
  for (const [id, advisory] of all) {
    const scope = shipped.has(id) ? 'runtime' : 'dev'
    const runtimeAdvisory = shipped.get(id)
    const severity =
      runtimeAdvisory &&
      rank(runtimeAdvisory.severity) > rank(advisory.severity)
        ? runtimeAdvisory.severity
        : advisory.severity
    const item = {
      ...advisory,
      severity,
      packages: [
        ...new Set([
          ...advisory.packages,
          ...(runtimeAdvisory?.packages ?? []),
        ]),
      ].sort(),
      scope,
    }
    const block =
      scope === 'runtime'
        ? rank(item.severity) >= SEVERITY_RANK.moderate
        : rank(item.severity) >= SEVERITY_RANK.critical
    ;(block ? blocking : warnings).push(item)
  }
  // An advisory only present in the runtime report would mean the two runs disagree.
  for (const id of shipped.keys()) {
    if (!all.has(id))
      throw new Error(`runtime advisory ${id} missing from the full npm report`)
  }
  return { blocking, warnings }
}

/** Classifies a `cargo audit --json` report. */
export function classifyCargoAudit(report) {
  const vulnerabilities = report?.vulnerabilities
  if (
    !Array.isArray(vulnerabilities?.list) ||
    !isCount(vulnerabilities.count) ||
    vulnerabilities.count !== vulnerabilities.list.length ||
    vulnerabilities.found !== Boolean(vulnerabilities.count)
  ) {
    throw new Error('cargo audit report has no vulnerabilities section')
  }
  if (!isRecord(report.warnings))
    throw new Error('cargo audit report has no warnings section')
  const settings = report.settings
  if (
    !isRecord(settings) ||
    ['ignore', 'target_arch', 'target_os'].some(
      (key) => !Array.isArray(settings[key]) || settings[key].length,
    ) ||
    settings.severity !== null ||
    !Array.isArray(settings.informational_warnings) ||
    ['unmaintained', 'unsound', 'notice'].some(
      (kind) => !settings.informational_warnings.includes(kind),
    )
  )
    throw new Error('cargo audit settings suppress or filter findings')

  function validateFinding(finding, advisoryRequired) {
    if (
      !isText(finding?.package?.name) ||
      !isText(finding?.package?.version) ||
      ((advisoryRequired || finding.advisory != null) &&
        (!isText(finding.advisory?.id) || !isText(finding.advisory?.title)))
    )
      throw new Error('cargo audit report contains a malformed finding')
  }
  vulnerabilities.list.forEach((v) => validateFinding(v, true))
  const blocking = report.vulnerabilities.list.map((v) => ({
    id: v.advisory.id,
    title: v.advisory.title,
    severity: 'vulnerability',
    packages: [`${v.package.name} ${v.package.version}`],
    scope: 'rust',
  }))
  const warnings = []
  for (const [kind, list] of Object.entries(report.warnings)) {
    if (!Array.isArray(list))
      throw new Error(`cargo audit malformed ${kind} warnings`)
    for (const w of list) {
      validateFinding(w, kind !== 'yanked')
      warnings.push({
        id: w.advisory?.id ?? `${kind}:${w.package.name}@${w.package.version}`,
        title: w.advisory?.title ?? kind,
        severity: kind,
        packages: [`${w.package.name} ${w.package.version}`],
        scope: 'rust',
      })
    }
  }
  return { blocking, warnings }
}

function run(command, args) {
  if (command === 'cargo') {
    // cargo-audit 0.22.2 auto-loads these files, but its JSON does not expose database or yank
    // settings. Refuse ambient config instead of accepting a report from a silently reduced audit.
    const cargoHome = process.env.CARGO_HOME || resolve(homedir(), '.cargo')
    for (const path of [
      resolve('.cargo', 'audit.toml'),
      resolve(cargoHome, 'audit.toml'),
    ]) {
      try {
        lstatSync(path)
      } catch (error) {
        if (error.code === 'ENOENT') continue
        throw new Error(
          `cannot inspect cargo-audit configuration: ${error.message}`,
        )
      }
      throw new Error(
        `unsupported cargo-audit configuration ${path}; shared policy requires upstream defaults`,
      )
    }
  }
  const result = spawnSync(command, args, {
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  })
  if (result.error)
    throw new Error(`${command} could not start: ${result.error.message}`)
  if (result.signal)
    throw new Error(`${command} ${args.join(' ')} died by ${result.signal}`)
  if (result.status !== 0 && result.status !== 1)
    throw new Error(`${command} audit failed with exit status ${result.status}`)
  // cargo-audit can emit a report and still report failed yank lookups on stderr, even with 0.
  if (
    /^\s*(?:npm (?:ERR!|error)|error(?:\[.*?\])?:)/im.test(
      stripVTControlCharacters(result.stderr),
    )
  )
    throw new Error(`${command} audit reported an operational error on stderr`)
  const report = parseAuditJson(`${command} audit`, result.stdout)
  if (command === 'npm') npmAdvisories(report, 'npm audit')
  else classifyCargoAudit(report)
  // npm 10 (--audit-level=low) and cargo-audit 0.22.2 exit 1 for findings, 0 for none.
  // cargo operational errors may exit 1 or 2. A valid-looking empty report cannot explain 1.
  const findings =
    command === 'npm'
      ? Object.keys(SEVERITY_RANK).some(
          (s) =>
            rank(s) >= SEVERITY_RANK.low &&
            report.metadata.vulnerabilities[s] > 0,
        )
      : report.vulnerabilities.found
  const expected = findings ? 1 : 0
  if (result.status !== expected)
    throw new Error(
      `${command} audit exit status ${result.status} disagrees with report (expected ${expected})`,
    )
  return report
}

function render(title, { blocking, warnings }) {
  const lines = [`### ${title}`, '']
  if (blocking.length === 0 && warnings.length === 0) {
    lines.push('No advisories.')
  } else {
    lines.push(
      '| Policy | Scope | Severity | Advisory | Packages |',
      '| --- | --- | --- | --- | --- |',
    )
    for (const [policy, list] of [
      ['BLOCKING', blocking],
      ['warning', warnings],
    ]) {
      for (const a of list) {
        lines.push(
          `| ${policy} | ${a.scope} | ${a.severity} | ${a.id} ${a.title.replaceAll('|', '/')} | ${a.packages.join(', ')} |`,
        )
      }
    }
  }
  return `${lines.join('\n')}\n`
}

function main(mode) {
  let title
  let verdict
  if (mode === 'npm') {
    title = 'npm audit'
    verdict = classifyNpmAudit(
      // Explicit authority/online flags and includes override ambient npm config. Both reports
      // must query npm's registry for the full/runtime graphs, even with a project .npmrc.
      run('npm', [
        'audit',
        '--json',
        '--audit-level=low',
        '--registry=https://registry.npmjs.org/',
        '--offline=false',
        '--include=prod',
        '--include=optional',
        '--include=peer',
        '--include=dev',
      ]),
      run('npm', [
        'audit',
        '--json',
        '--audit-level=low',
        '--registry=https://registry.npmjs.org/',
        '--offline=false',
        '--include=prod',
        '--include=optional',
        '--include=peer',
        '--omit=dev',
      ]),
    )
  } else if (mode === 'cargo') {
    title = 'cargo audit (RustSec)'
    verdict = classifyCargoAudit(run('cargo', ['audit', '--json']))
  } else {
    throw new Error('usage: dependency-audit-policy.mjs npm|cargo')
  }
  const summary = render(title, verdict)
  process.stdout.write(summary)
  if (process.env.GITHUB_STEP_SUMMARY)
    appendFileSync(process.env.GITHUB_STEP_SUMMARY, summary)
  const annotate = process.env.GITHUB_ACTIONS === 'true'
  for (const a of verdict.warnings) {
    if (annotate)
      console.log(
        `::warning title=${title}::${a.scope} ${a.severity} ${a.id} (${a.packages.join(', ')})`,
      )
  }
  for (const a of verdict.blocking) {
    console.error(
      `${annotate ? `::error title=${title}::` : 'BLOCKING: '}${a.scope} ${a.severity} ${a.id} (${a.packages.join(', ')})`,
    )
  }
  if (verdict.blocking.length > 0) {
    console.error(`${title}: ${verdict.blocking.length} blocking advisories`)
    process.exit(1)
  }
  console.log(
    `${title}: ok (${verdict.warnings.length} non-blocking advisories reported)`,
  )
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv[2])
  } catch (error) {
    console.error(`dependency-audit-policy: ${error.message}`)
    process.exit(1)
  }
}
