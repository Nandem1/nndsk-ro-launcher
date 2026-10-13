#!/usr/bin/env node
// Dependency audit policy shared by dependency-audit.yml and release.yml.
//
// The workflow must never turn green because an audit command crashed or because its exit code
// was swallowed. This script runs the audit itself, refuses any report it cannot parse, and then
// applies an explicit policy:
//
// - npm runtime graph (`--omit=dev`, what ships inside the AppImage webview): moderate or worse
//   blocks; low is reported.
// - npm dev/build toolchain only: critical blocks; high/moderate/low are reported as warnings so
//   they stay visible without making every toolchain ReDoS a release blocker.
// - RustSec: every vulnerability blocks (Cargo.lock is the shipped binary plus its build graph).
//   Informational warnings (unmaintained, unsound, yanked) are reported, never hidden.
import { spawnSync } from 'node:child_process'
import { appendFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const SEVERITY_RANK = { info: 0, low: 1, moderate: 2, high: 3, critical: 4 }

function rank(severity) {
  const value = SEVERITY_RANK[severity]
  if (value === undefined)
    throw new Error(`unknown npm severity ${JSON.stringify(severity)}`)
  return value
}

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
    !report ||
    typeof report.vulnerabilities !== 'object' ||
    typeof report.metadata?.vulnerabilities !== 'object'
  ) {
    throw new Error(
      `${label}: npm audit report has no vulnerabilities/metadata section`,
    )
  }
  const advisories = new Map()
  for (const [pkg, entry] of Object.entries(report.vulnerabilities)) {
    for (const via of entry.via ?? []) {
      if (typeof via !== 'object') continue
      const id = via.url || `${via.source}`
      const known = advisories.get(id)
      if (known) {
        known.packages.add(pkg)
        continue
      }
      rank(via.severity)
      advisories.set(id, {
        id,
        title: via.title ?? '(no title)',
        severity: via.severity,
        packages: new Set([pkg]),
      })
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
    const item = { ...advisory, packages: [...advisory.packages].sort(), scope }
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
  if (!report || typeof report.vulnerabilities?.list !== 'object') {
    throw new Error('cargo audit report has no vulnerabilities section')
  }
  const blocking = report.vulnerabilities.list.map((v) => ({
    id: v.advisory.id,
    title: v.advisory.title,
    severity: 'vulnerability',
    packages: [`${v.package.name} ${v.package.version}`],
    scope: 'rust',
  }))
  const warnings = []
  for (const [kind, list] of Object.entries(report.warnings ?? {})) {
    for (const w of list) {
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
  const result = spawnSync(command, args, {
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  })
  if (result.error)
    throw new Error(`${command} could not start: ${result.error.message}`)
  if (result.signal)
    throw new Error(`${command} ${args.join(' ')} died by ${result.signal}`)
  // Both audits exit non-zero when they find advisories; the parsed report is the authority and
  // an unparseable report fails closed above.
  return result.stdout
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
      parseAuditJson('npm audit', run('npm', ['audit', '--json'])),
      parseAuditJson(
        'npm audit --omit=dev',
        run('npm', ['audit', '--json', '--omit=dev']),
      ),
    )
  } else if (mode === 'cargo') {
    title = 'cargo audit (RustSec)'
    verdict = classifyCargoAudit(
      parseAuditJson('cargo audit', run('cargo', ['audit', '--json'])),
    )
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
