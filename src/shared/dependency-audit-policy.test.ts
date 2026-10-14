import { describe, expect, it } from 'vitest'
import { spawnSync } from 'node:child_process'
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import {
  classifyCargoAudit,
  classifyNpmAudit,
  parseAuditJson,
} from '../../scripts/dependency-audit-policy.mjs'

function npmReport(entries: Record<string, [string, string][]>) {
  const vulnerabilities: Record<string, unknown> = {}
  const counts: Record<string, number> = {
    info: 0,
    low: 0,
    moderate: 0,
    high: 0,
    critical: 0,
    total: 0,
  }
  const ranks = ['info', 'low', 'moderate', 'high', 'critical']
  for (const [pkg, advisories] of Object.entries(entries)) {
    const severity = advisories.reduce(
      (a, [, b]) => (ranks.indexOf(a) > ranks.indexOf(b) ? a : b),
      'info',
    )
    vulnerabilities[pkg] = {
      name: pkg,
      severity,
      via: advisories.map(([url, severity]) => ({
        source: [...url].reduce((a, c) => a * 31 + c.charCodeAt(0), 0) >>> 0,
        url,
        severity,
        title: url,
      })),
    }
    counts[severity]++
    counts.total++
  }
  return {
    auditReportVersion: 2,
    vulnerabilities,
    metadata: { vulnerabilities: counts },
  }
}

function cargoReport(list: unknown[] = [], warnings = {}) {
  return {
    settings: {
      ignore: [],
      target_arch: [],
      target_os: [],
      severity: null,
      informational_warnings: ['unmaintained', 'unsound', 'notice'],
    },
    vulnerabilities: { found: list.length > 0, count: list.length, list },
    warnings,
  }
}

const rustVulnerability = {
  advisory: { id: 'RUSTSEC-1', title: 'bad' },
  package: { name: 'rustls', version: '0.1.0' },
}

describe('classifyNpmAudit', () => {
  it('blocks moderate runtime advisories but only warns for dev-only highs', () => {
    const full = npmReport({
      react: [['GHSA-runtime', 'moderate']],
      braces: [['GHSA-dev', 'high']],
    })
    const runtime = npmReport({ react: [['GHSA-runtime', 'moderate']] })
    const verdict = classifyNpmAudit(full, runtime)
    expect(verdict.blocking.map((a) => a.id)).toEqual(['GHSA-runtime'])
    expect(verdict.warnings.map((a) => [a.id, a.scope])).toEqual([
      ['GHSA-dev', 'dev'],
    ])
  })

  it('blocks a critical advisory even when it is dev-only', () => {
    const full = npmReport({ toolchain: [['GHSA-crit', 'critical']] })
    const verdict = classifyNpmAudit(full, npmReport({}))
    expect(verdict.blocking.map((a) => a.scope)).toEqual(['dev'])
  })

  it('reports low runtime advisories without blocking', () => {
    const report = npmReport({ zustand: [['GHSA-low', 'low']] })
    const verdict = classifyNpmAudit(report, report)
    expect(verdict.blocking).toEqual([])
    expect(verdict.warnings.map((a) => a.scope)).toEqual(['runtime'])
  })

  it('fails closed on a report without the expected sections', () => {
    expect(() => classifyNpmAudit({}, npmReport({}))).toThrow(/metadata/)
  })

  it('fails closed on an unknown severity', () => {
    const odd = npmReport({ x: [['GHSA-x', 'catastrophic']] })
    expect(() => classifyNpmAudit(odd, npmReport({}))).toThrow(/severity/)
  })

  it('fails closed if runtime contains an advisory absent from full', () => {
    expect(() =>
      classifyNpmAudit(npmReport({}), npmReport({ x: [['GHSA-x', 'low']] })),
    ).toThrow(/missing from the full/)
  })

  it('uses the strongest severity for duplicate IDs and across both reports', () => {
    const full = npmReport({
      x: [
        ['GHSA-x', 'low'],
        ['GHSA-x', 'critical'],
      ],
    })
    expect(classifyNpmAudit(full, full).blocking[0].severity).toBe('critical')
    const lower = npmReport({ x: [['GHSA-x', 'low']] })
    const higher = npmReport({ renamed: [['GHSA-x', 'moderate']] })
    expect(classifyNpmAudit(lower, higher).blocking[0]).toMatchObject({
      severity: 'moderate',
      packages: ['renamed', 'x'],
      scope: 'runtime',
    })
  })

  it('validates every duplicate severity, including prototype property names', () => {
    for (const severity of ['catastrophic', 'constructor', '__proto__']) {
      const report = npmReport({
        x: [
          ['GHSA-x', 'low'],
          ['GHSA-x', severity],
        ],
      })
      expect(() => classifyNpmAudit(report, report)).toThrow(/severity/)
    }
  })

  it('follows meta-vulnerability references and retains runtime scope', () => {
    const report = npmReport({ leaf: [['GHSA-shared', 'moderate']] })
    report.vulnerabilities.parent = {
      name: 'parent',
      severity: 'moderate',
      via: ['leaf'],
    }
    report.metadata.vulnerabilities.moderate++
    report.metadata.vulnerabilities.total++
    expect(classifyNpmAudit(report, report).blocking[0].packages).toEqual([
      'leaf',
      'parent',
    ])
  })

  it('matches numeric advisory identity despite URL and package representation changes', () => {
    const full = npmReport({
      build: [['GHSA-shared', 'moderate']],
      frontend: [['GHSA-shared', 'moderate']],
    })
    const runtime = npmReport({ frontend: [['GHSA-shared', 'moderate']] })
    const entry = runtime.vulnerabilities.frontend as { via: { url: string }[] }
    entry.via[0].url = 'https://github.com/advisories/GHSA-shared'
    expect(classifyNpmAudit(full, runtime).blocking[0]).toMatchObject({
      scope: 'runtime',
      packages: ['build', 'frontend'],
    })
  })

  it('rejects partial, inconsistent or unsupported npm reports', () => {
    const clean = npmReport({})
    const partials = [
      { ...clean, auditReportVersion: 1 },
      { ...clean, vulnerabilities: [] },
      { ...clean, vulnerabilities: null },
      { ...clean, metadata: { vulnerabilities: {} } },
      {
        ...clean,
        metadata: {
          vulnerabilities: { ...clean.metadata.vulnerabilities, total: 1 },
        },
      },
      {
        ...clean,
        vulnerabilities: { x: { name: 'x', severity: 'critical', via: [] } },
      },
      {
        ...clean,
        vulnerabilities: {
          x: { name: 'x', severity: 'critical', via: ['missing'] },
        },
      },
      {
        ...clean,
        vulnerabilities: { x: { name: 'x', severity: 'critical', via: ['x'] } },
      },
      { ...clean, vulnerabilities: { x: { via: [{}] } } },
    ]
    for (const report of partials) {
      expect(() => classifyNpmAudit(report, clean)).toThrow()
    }
  })
})

describe('parseAuditJson', () => {
  it('rejects non-JSON output such as a crashed audit', () => {
    expect(() => parseAuditJson('npm audit', 'npm ERR! network')).toThrow(
      /not JSON/,
    )
  })

  it('rejects an error object returned instead of a report', () => {
    expect(() =>
      parseAuditJson('npm audit', JSON.stringify({ error: { code: 'E500' } })),
    ).toThrow(/E500/)
  })
})

describe('classifyCargoAudit', () => {
  it('blocks every RustSec vulnerability and reports warnings', () => {
    const verdict = classifyCargoAudit(
      cargoReport([rustVulnerability], {
        unmaintained: [
          {
            advisory: { id: 'RUSTSEC-2', title: 'old' },
            package: { name: 'unic-common', version: '0.9.0' },
          },
        ],
        unsound: [
          {
            advisory: { id: 'RUSTSEC-3', title: 'unsound API' },
            package: { name: 'glib', version: '0.18.5' },
          },
        ],
        yanked: [{ package: { name: 'chacha20', version: '0.10.1' } }],
      }),
    )
    expect(verdict.blocking.map((a) => a.id)).toEqual(['RUSTSEC-1'])
    expect(verdict.warnings.map((a) => a.id)).toEqual([
      'RUSTSEC-2',
      'RUSTSEC-3',
      'yanked:chacha20@0.10.1',
    ])
  })

  it('fails closed when the vulnerabilities section is missing', () => {
    expect(() => classifyCargoAudit({ warnings: {} })).toThrow(
      /vulnerabilities/,
    )
  })

  it('rejects inconsistent counts, malformed lists and dropped warning sections', () => {
    const clean = cargoReport()
    for (const report of [
      { ...clean, vulnerabilities: { found: true, count: 1, list: [] } },
      { ...clean, vulnerabilities: { found: false, count: 0, list: {} } },
      { ...clean, vulnerabilities: { found: false, count: 0, list: null } },
      { ...clean, warnings: undefined },
      { ...clean, warnings: { unsound: {} } },
      {
        ...clean,
        warnings: { unmaintained: [{ package: { name: 'x', version: '1' } }] },
      },
      cargoReport([{}]),
    ])
      expect(() => classifyCargoAudit(report)).toThrow()
  })

  it('rejects filtered or suppressed RustSec reports', () => {
    const clean = cargoReport()
    for (const settings of [
      { ...clean.settings, ignore: ['RUSTSEC-1'] },
      { ...clean.settings, target_os: ['windows'] },
      { ...clean.settings, severity: 'critical' },
      { ...clean.settings, informational_warnings: [] },
    ])
      expect(() => classifyCargoAudit({ ...clean, settings })).toThrow(
        /settings/,
      )
  })
})

describe('audit subprocess boundary', () => {
  const script = join(process.cwd(), 'scripts/dependency-audit-policy.mjs')

  function probe(
    mode: string,
    stdout: string,
    status: number | 'signal',
    stderr = '',
    configuration?: 'project' | 'user',
  ) {
    const directory = mkdtempSync(join(tmpdir(), 'ro-audit-test-'))
    const summary = join(directory, 'summary.md')
    const cargoHome = join(directory, 'cargo-home')
    try {
      if (configuration) {
        const configDirectory =
          configuration === 'project' ? join(directory, '.cargo') : cargoHome
        mkdirSync(configDirectory)
        writeFileSync(
          join(configDirectory, 'audit.toml'),
          '[yanked]\nenabled = false\n',
        )
      }
      const executable = mode === 'cargo' ? 'cargo' : 'npm'
      const source = `#!${process.execPath}\nprocess.stdout.write(${JSON.stringify(stdout)});\nprocess.stderr.write(${JSON.stringify(stderr)});\n${status === 'signal' ? "process.kill(process.pid, 'SIGTERM')" : `process.exit(${status})`}\n`
      writeFileSync(join(directory, executable), source, { mode: 0o700 })
      const result = spawnSync(process.execPath, [script, mode], {
        encoding: 'utf8',
        cwd: directory,
        env: {
          ...process.env,
          PATH: directory,
          GITHUB_ACTIONS: 'true',
          GITHUB_STEP_SUMMARY: summary,
          CARGO_HOME: cargoHome,
        },
      })
      let markdown = ''
      if (result.status === 0) markdown = readFileSync(summary, 'utf8')
      return { ...result, markdown }
    } finally {
      rmSync(directory, { recursive: true, force: true })
    }
  }

  it.each(['npm', 'cargo'])(
    'fails closed on missing %s, signal death and malformed JSON',
    (mode) => {
      const missing = spawnSync(process.execPath, [script, mode], {
        encoding: 'utf8',
        env: { ...process.env, PATH: '/nonexistent-ro-audit-test' },
      })
      expect(missing.status).toBe(1)
      expect(missing.stderr).toMatch(/could not start/)
      expect(probe(mode, '{}', 'signal').stderr).toMatch(/SIGTERM/)
      expect(probe(mode, '{', 0).stderr).toMatch(/not JSON/)
    },
  )

  it.each(['npm', 'cargo'])(
    'rejects %s operational nonzero status with valid-looking clean JSON',
    (mode) => {
      const clean = JSON.stringify(
        mode === 'npm' ? npmReport({}) : cargoReport(),
      )
      for (const status of [1, 2, 42])
        expect(probe(mode, clean, status).status).toBe(1)
    },
  )

  it('accepts npm exit 1 for dev high findings, and keeps warnings visible', () => {
    const high = JSON.stringify(npmReport({ tool: [['GHSA-high', 'high']] }))
    // Emit full only; the fake runtime invocation returns a clean report.
    const fullOnly = JSON.parse(high)
    const directory = mkdtempSync(join(tmpdir(), 'ro-audit-scope-'))
    try {
      writeFileSync(
        join(directory, 'npm'),
        `#!${process.execPath}\nconst runtime=process.argv.includes('--omit=dev');process.stdout.write(JSON.stringify(runtime?${JSON.stringify(npmReport({}))}:${JSON.stringify(fullOnly)}));process.exit(runtime?0:1)\n`,
        { mode: 0o700 },
      )
      const result = spawnSync(process.execPath, [script, 'npm'], {
        encoding: 'utf8',
        env: {
          ...process.env,
          PATH: directory,
          GITHUB_ACTIONS: 'true',
          GITHUB_STEP_SUMMARY: join(directory, 'summary'),
        },
      })
      expect(result.status).toBe(0)
      expect(result.stdout).toMatch(/::warning.*GHSA-high/)
      expect(readFileSync(join(directory, 'summary'), 'utf8')).toMatch(
        /warning.*dev.*high/,
      )
    } finally {
      rmSync(directory, { recursive: true, force: true })
    }
  })

  it('blocks synthetic Rust vulnerabilities even if an audit exits zero', () => {
    const report = JSON.stringify(cargoReport([rustVulnerability]))
    expect(probe('cargo', report, 1).status).toBe(1)
    expect(probe('cargo', report, 0).status).toBe(1)
  })

  it('reports Rust warnings in annotations and the step summary', () => {
    const warning = {
      advisory: { id: 'RUSTSEC-W', title: 'unsound' },
      package: { name: 'glib', version: '0.18.5' },
    }
    const result = probe(
      'cargo',
      JSON.stringify(cargoReport([], { unsound: [warning] })),
      0,
    )
    expect(result.status).toBe(0)
    expect(result.stdout).toMatch(/::warning.*RUSTSEC-W/)
    expect(result.markdown).toMatch(/warning.*unsound.*RUSTSEC-W/)
  })

  it('rejects operational stderr even beside a valid advisory report', () => {
    expect(
      probe(
        'npm',
        JSON.stringify(npmReport({ x: [['GHSA-x', 'low']] })),
        1,
        'npm error network failure',
      ).status,
    ).toBe(1)
    expect(
      probe('cargo', JSON.stringify(cargoReport([rustVulnerability])), 2)
        .status,
    ).toBe(1)
  })

  it('rejects incomplete yank lookups even after a clean report and exit zero', () => {
    expect(
      probe(
        'cargo',
        JSON.stringify(cargoReport()),
        0,
        "\u001b[31merror:\u001b[0m couldn't check if the package is yanked",
      ).status,
    ).toBe(1)
  })

  it.each(['project', 'user'] as const)(
    'rejects %s audit config that can silently disable yank checks',
    (configuration) => {
      const result = probe(
        'cargo',
        JSON.stringify(cargoReport()),
        0,
        '',
        configuration,
      )
      expect(result.status).toBe(1)
      expect(result.stderr).toMatch(/configuration/)
    },
  )
})
