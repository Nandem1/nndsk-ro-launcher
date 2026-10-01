import { describe, expect, it } from 'vitest'
import { spawnSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { sidecarOrderError } from '../../scripts/release-sidecar-order.mjs'

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

describe('sidecarOrderError', () => {
  it('rejects the failing v0.1.0 workflow that only built --release sidecars before clippy', () => {
    const broken = `
      - name: Build sidecars
        run: cargo build -p ro-inputd -p ro-sessiond --release
      - run: cargo clippy --workspace --all-targets --all-features -- -D warnings
      - uses: tauri-apps/tauri-action@v1
`
    expect(sidecarOrderError(broken)).toMatch(/debug sidecars/)
  })

  it('accepts the current release.yml order', () => {
    const yaml = readFileSync(
      join(root, '.github/workflows/release.yml'),
      'utf8',
    )
    expect(sidecarOrderError(yaml)).toBeNull()
    expect(yaml).toMatch(/uploadUpdaterJson:\s*true/)
    expect(yaml).not.toMatch(/includeUpdaterJson/)
  })
})
