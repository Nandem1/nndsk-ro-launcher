import { describe, expect, it } from 'vitest'
import { spawnSync } from 'node:child_process'
import { join } from 'node:path'

const root = process.cwd()
const script = join(root, 'scripts/check-release-invariants.mjs')

describe('check-release-invariants', () => {
  it('passes on the current repository tree', () => {
    const result = spawnSync(process.execPath, [script], {
      cwd: root,
      encoding: 'utf8',
    })
    expect(result.status).toBe(0)
    expect(result.stdout).toMatch(/ok/)
  })
})
