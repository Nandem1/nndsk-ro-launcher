import { describe, expect, it } from 'vitest'
import corpus from '../../contract-fixtures/ipc-tagged-enums-v1.json'
import { compatibilityLine } from '../features/settings/advanced.logic'
import type { CompatibilityAssessmentIpc, CompatibilityStatus } from './types'

// The fixture is produced and verified by Rust (src-tauri/src/models/ipc_golden.rs).
// Field-name conformance to the declared TS types is enforced by tsc in
// ipcGolden.contract.ts; these tests cover runtime consumption and naming.

type Kinded = { kind: string } & Record<string, unknown>

describe('Rust-produced tagged-enum golden corpus', () => {
  it('reads CompatibilityAssessmentIpc evidenceId from the Rust wire shape', () => {
    const { validated, experimental, incompatible, unknown } =
      corpus.compatibilityAssessment
    expect(validated).toEqual({
      kind: 'validated',
      evidenceId: 'evidence-validated-1',
    })

    const status: CompatibilityStatus = {
      assessment: validated as CompatibilityAssessmentIpc,
      gepardFileVersion: '1.0',
    } as CompatibilityStatus
    expect(compatibilityLine(status).hint).toBe('evidence-validated-1 · 1.0')

    expect(experimental.evidenceId).toBe('evidence-experimental-1')
    expect(incompatible.evidenceId).toBe('evidence-incompatible-1')
    expect(unknown).toEqual({ kind: 'unknown' })
  })

  it('has a snake_case-free wire shape', () => {
    const offenders: string[] = []
    const walk = (value: unknown, path: string) => {
      if (Array.isArray(value)) {
        value.forEach((item, i) => walk(item, `${path}[${i}]`))
      } else if (value && typeof value === 'object') {
        for (const [key, child] of Object.entries(value)) {
          if (key.includes('_')) offenders.push(`${path}.${key}`)
          walk(child, `${path}.${key}`)
        }
      }
    }
    walk(corpus, 'corpus')
    expect(offenders).toEqual([])
  })

  it('names each fixture case after its kind', () => {
    for (const group of Object.values(corpus)) {
      for (const [name, value] of Object.entries(group)) {
        expect((value as Kinded).kind).toBe(name)
      }
    }
  })
})
