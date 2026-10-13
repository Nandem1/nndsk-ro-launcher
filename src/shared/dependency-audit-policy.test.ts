import { describe, expect, it } from 'vitest'
import {
  classifyCargoAudit,
  classifyNpmAudit,
  parseAuditJson,
} from '../../scripts/dependency-audit-policy.mjs'

function npmReport(entries: Record<string, [string, string][]>) {
  const vulnerabilities: Record<string, unknown> = {}
  for (const [pkg, advisories] of Object.entries(entries)) {
    vulnerabilities[pkg] = {
      via: advisories.map(([url, severity]) => ({ url, severity, title: url })),
    }
  }
  return { vulnerabilities, metadata: { vulnerabilities: {} } }
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
    const verdict = classifyCargoAudit({
      vulnerabilities: {
        list: [
          {
            advisory: { id: 'RUSTSEC-1', title: 'bad' },
            package: { name: 'rustls', version: '0.1.0' },
          },
        ],
      },
      warnings: {
        unmaintained: [
          {
            advisory: { id: 'RUSTSEC-2', title: 'old' },
            package: { name: 'unic-common', version: '0.9.0' },
          },
        ],
        yanked: [{ package: { name: 'chacha20', version: '0.10.1' } }],
      },
    })
    expect(verdict.blocking.map((a) => a.id)).toEqual(['RUSTSEC-1'])
    expect(verdict.warnings.map((a) => a.id)).toEqual([
      'RUSTSEC-2',
      'yanked:chacha20@0.10.1',
    ])
  })

  it('fails closed when the vulnerabilities section is missing', () => {
    expect(() => classifyCargoAudit({ warnings: {} })).toThrow(
      /vulnerabilities/,
    )
  })
})
