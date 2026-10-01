import { describe, expect, it } from 'vitest'
import { spawnSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import {
  applyVersion,
  nextVersion,
  parseSemver,
  readAppVersion,
} from '../../scripts/bump-version.mjs'

const root = process.cwd()
const script = join(root, 'scripts/bump-version.mjs')

function loadFiles() {
  return {
    packageJson: JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')),
    packageLock: readFileSync(join(root, 'package-lock.json'), 'utf8'),
    tauriConf: JSON.parse(
      readFileSync(join(root, 'src-tauri/tauri.conf.json'), 'utf8'),
    ),
    cargoToml: readFileSync(join(root, 'src-tauri/Cargo.toml'), 'utf8'),
    cargoLock: readFileSync(join(root, 'Cargo.lock'), 'utf8'),
  }
}

describe('nextVersion', () => {
  it('bumps patch minor and major from 0.1.0', () => {
    expect(nextVersion('0.1.0', 'patch')).toBe('0.1.1')
    expect(nextVersion('0.1.0', 'minor')).toBe('0.2.0')
    expect(nextVersion('0.1.0', 'major')).toBe('1.0.0')
    expect(nextVersion('0.1.0', '0.1.2')).toBe('0.1.2')
  })

  it('rejects the same version and a non X.Y.Z spec', () => {
    expect(() => nextVersion('0.1.0', '0.1.0')).toThrow(/already at 0.1.0/)
    expect(() => nextVersion('0.1.0', '1.2')).toThrow(/expected patch/)
    expect(parseSemver('v0.1.0')).toBeNull()
  })
})

describe('readAppVersion', () => {
  it('reads one version from the real tree', () => {
    const version = readAppVersion(loadFiles())
    expect(parseSemver(version)).not.toBeNull()
  })
})

describe('applyVersion', () => {
  it('rewrites every tracked source without touching dependency versions', () => {
    const current = readAppVersion(loadFiles())
    const next = nextVersion(current, 'patch')
    const files = applyVersion(loadFiles(), current, next)
    expect(files.packageJson.version).toBe(next)
    expect(files.tauriConf.version).toBe(next)
    expect(files.cargoToml).toMatch(new RegExp(`^version = "${next}"$`, 'm'))
    expect(files.cargoToml).toContain('tauri-build = { version = "2"')
    expect(files.cargoLock).toContain(
      `name = "ro-launcher"\nversion = "${next}"`,
    )
    expect(files.packageLock).toContain(`"version": "${next}"`)
    expect(files.packageLock).not.toContain(
      `"name": "ro-launcher",\n  "version": "${current}"`,
    )
  })
})

describe('bump-version CLI', () => {
  it('prints the current version and dry-runs a patch without writing', () => {
    const current = readAppVersion(loadFiles())
    const next = nextVersion(current, 'patch')
    const show = spawnSync(process.execPath, [script, '--show'], {
      cwd: root,
      encoding: 'utf8',
    })
    expect(show.status).toBe(0)
    expect(show.stdout.trim()).toBe(current)

    const dry = spawnSync(process.execPath, [script, '--dry-run', 'patch'], {
      cwd: root,
      encoding: 'utf8',
    })
    expect(dry.status).toBe(0)
    expect(dry.stdout.trim()).toBe(
      `bump-version: dry-run ${current} -> ${next}`,
    )
    expect(
      JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version,
    ).toBe(current)
  })
})
