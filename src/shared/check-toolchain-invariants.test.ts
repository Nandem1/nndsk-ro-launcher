import { describe, expect, it } from 'vitest'
import { spawnSync } from 'node:child_process'
import { mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import {
  checkToolchainInvariants,
  rustToolchainVersion,
  toolchainInvariantErrors,
} from '../../scripts/check-toolchain-invariants.mjs'
import { unpinnedActionErrors } from '../../scripts/release-workflow-policy.mjs'

const root = process.cwd()
const files: Record<string, string> = {}
for (const path of ['rust-toolchain.toml', '.nvmrc', 'package.json']) {
  files[path] = readFileSync(join(root, path), 'utf8')
}
function walk(path: string) {
  for (const entry of readdirSync(join(root, path), { withFileTypes: true })) {
    const relative = `${path}/${entry.name}`
    if (entry.isDirectory()) walk(relative)
    else if (/\.ya?ml$/.test(relative))
      files[relative] = readFileSync(join(root, relative), 'utf8')
  }
}
walk('.github/workflows')
walk('.github/actions')

const ci = '.github/workflows/ci.yml'
const release = '.github/workflows/release.yml'
const audit = '.github/workflows/dependency-audit.yml'
const rustInstall = 'run: rustup toolchain install --no-self-update'

function mutate(path: string, before: string, after: string) {
  expect(files[path]).toContain(before)
  return { ...files, [path]: files[path].replace(before, after) }
}

describe('repository toolchain contract', () => {
  it('keeps the approved pins separate from the Node compatibility requirement', () => {
    expect(rustToolchainVersion(files['rust-toolchain.toml'])).toBe('1.98.1')
    expect(files['.nvmrc']).toBe('22.23.3\n')
    expect(JSON.parse(files['package.json']).engines.node).toBe('>=22')
    expect(toolchainInvariantErrors(files)).toEqual([])
    expect(checkToolchainInvariants(root)).toEqual([])
  })

  it('passes through the standalone CLI', () => {
    const result = spawnSync(
      process.execPath,
      ['scripts/check-toolchain-invariants.mjs'],
      { cwd: root, encoding: 'utf8' },
    )
    expect(result.status, result.stderr).toBe(0)
    expect(result.stdout).toContain('ok')
  })

  it.each([ci, release, audit])(
    'rejects stable or an independent exact Rust selector in %s',
    (path) => {
      for (const version of ['stable', '1.99.0']) {
        const changed = mutate(
          path,
          rustInstall,
          `run: rustup toolchain install ${version}`,
        )
        expect(toolchainInvariantErrors(changed).join('\n')).toMatch(
          /must install Rust|independent Rust selector/,
        )
        const action = mutate(
          path,
          rustInstall,
          `uses: dtolnay/rust-toolchain@686976e191b89faba57d3206551f0f330d8cb249\n        with:\n          toolchain: ${version}`,
        )
        expect(toolchainInvariantErrors(action).join('\n')).toContain(
          'independent Rust selector',
        )
      }
    },
  )

  it.each(['stable', '1.98', '', '1.98.1\nchannel = "1.99.0"'])(
    'rejects malformed Rust channel %j',
    (channel) => {
      const changed = mutate('rust-toolchain.toml', '1.98.1', channel)
      expect(toolchainInvariantErrors(changed).join('\n')).toContain(
        'rust-toolchain.toml',
      )
    },
  )

  it('rejects missing Rust authority and altered components/profile', () => {
    const missing = { ...files }
    delete missing['rust-toolchain.toml']
    expect(toolchainInvariantErrors(missing).join('\n')).toContain(
      'rust-toolchain.toml',
    )
    for (const [before, after] of [
      ['"clippy"', '"rust-src"'],
      ['"minimal"', '"default"'],
    ]) {
      expect(
        toolchainInvariantErrors(
          mutate('rust-toolchain.toml', before, after),
        ).join('\n'),
      ).toContain('rust-toolchain.toml')
    }
    const scratch = mkdtempSync(join(tmpdir(), 'ro-toolchain-policy-'))
    try {
      expect(() => checkToolchainInvariants(scratch)).toThrow(
        /rust-toolchain.toml: cannot read required authority/,
      )
    } finally {
      rmSync(scratch, { recursive: true })
    }
  })

  it.each([ci, release, audit])(
    'rejects independent Node versions in %s',
    (path) => {
      for (const selector of ['22', '22.23.2', 'latest']) {
        expect(
          toolchainInvariantErrors(
            mutate(
              path,
              'node-version-file: .nvmrc',
              `node-version: ${selector}`,
            ),
          ).join('\n'),
        ).toContain('independent Node/npm selector')
      }
      expect(
        toolchainInvariantErrors(
          mutate(
            path,
            'node-version-file: .nvmrc',
            'node-version-file: .node-version',
          ),
        ).join('\n'),
      ).toContain('must consume node-version-file: .nvmrc only')
    },
  )

  it.each(['22\n', 'latest\n', '', '22.23.3\n24.0.0\n'])(
    'rejects malformed Node pin %j',
    (pin) => {
      expect(
        toolchainInvariantErrors({ ...files, '.nvmrc': pin }).join('\n'),
      ).toContain('.nvmrc must contain one exact Node')
    },
  )

  it('retains >=22 compatibility instead of narrowing engines to the build pin', () => {
    expect(
      toolchainInvariantErrors(
        mutate('package.json', '"node": ">=22"', '"node": "22.23.3"'),
      ).join('\n'),
    ).toContain('>=22 compatibility contract')
    expect(
      toolchainInvariantErrors({ ...files, '.nvmrc': '20.19.0\n' }).join('\n'),
    ).toContain('must satisfy engines.node')
  })

  it.each([ci, release, audit])(
    'fails when %s is omitted from the inventory',
    (path) => {
      const missing = { ...files }
      delete missing[path]
      expect(toolchainInvariantErrors(missing).join('\n')).toContain(
        `${path}: required workflow missing`,
      )
    },
  )

  it('requires both cache producer and release consumer, plus the scheduled RustSec Node setup', () => {
    expect(
      toolchainInvariantErrors(
        mutate(ci, '  release-cache:', '  renamed-cache:'),
      ).join('\n'),
    ).toContain('required job release-cache')
    expect(
      toolchainInvariantErrors(
        mutate(release, '  build:', '  renamed-build:'),
      ).join('\n'),
    ).toContain('required job build')
    const setup =
      / {6}- uses: actions\/setup-node@[^\n]+\n {8}with:\n {10}node-version-file: \.nvmrc\n/g
    const changed = { ...files, [audit]: files[audit].replace(setup, '') }
    expect(toolchainInvariantErrors(changed).join('\n')).toContain(
      'rustsec: must set up Node',
    )
  })

  it('discovers additional workflows/jobs and refuses unrecognized structure', () => {
    const extra = '.github/workflows/new.yml'
    expect(
      toolchainInvariantErrors({
        ...files,
        [extra]:
          'jobs:\n  check:\n    steps:\n      - run: node scripts/check-design.mjs\n',
      }).join('\n'),
    ).toContain(`${extra}:check: must set up Node`)
    expect(
      toolchainInvariantErrors({
        ...files,
        [extra]: 'jobs: {check: {steps: []}}\n',
      }).join('\n'),
    ).toContain('unrecognized job structure')
    expect(
      toolchainInvariantErrors(mutate(ci, '  quality:', '    quality:')).join(
        '\n',
      ),
    ).toContain('required job quality')
  })

  it('fails closed on quoted job headers, duplicate steps and duplicate Node inputs', () => {
    const quoted = `${files[ci]}\n  'extra':\n    steps:\n      - run: node scripts/check-design.mjs\n`
    expect(
      toolchainInvariantErrors({ ...files, [ci]: quoted }).join('\n'),
    ).toContain('unrecognized job structure')
    const duplicateSteps = mutate(
      ci,
      '    steps:',
      '    steps:\n      - run: echo ignored\n    steps:',
    )
    expect(toolchainInvariantErrors(duplicateSteps).join('\n')).toContain(
      'exactly one literal steps list',
    )
    const duplicateInputs = mutate(
      ci,
      '          node-version-file: .nvmrc',
      '          node-version-file: .nvmrc\n        with:\n          cache: npm',
    )
    expect(toolchainInvariantErrors(duplicateInputs).join('\n')).toContain(
      'must consume node-version-file: .nvmrc only',
    )
  })

  it('rejects environment, conditional setup, directory and ordering escapes', () => {
    expect(
      toolchainInvariantErrors({
        ...files,
        [ci]: `env:\n  RUSTUP_TOOLCHAIN: stable\n${files[ci]}`,
      }).join('\n'),
    ).toContain('independent Rust selector')
    expect(
      toolchainInvariantErrors(
        mutate(ci, rustInstall, `if: false\n        ${rustInstall}`),
      ).join('\n'),
    ).toContain('unconditionally consume')
    expect(
      toolchainInvariantErrors(
        mutate(
          ci,
          rustInstall,
          `working-directory: /tmp\n        ${rustInstall}`,
        ),
      ).join('\n'),
    ).toContain('working-directory override')
    expect(
      toolchainInvariantErrors(
        mutate(
          ci,
          '      - name: Install repository Rust toolchain',
          '      - run: cargo --version\n      - name: Install repository Rust toolchain',
        ),
      ).join('\n'),
    ).toContain('must precede Rust commands')
  })

  it('keeps Rust out of sign/publish and disallows hidden language tooling in local actions', () => {
    expect(
      toolchainInvariantErrors(
        mutate(release, '  sign:\n', '  sign:\n    env:\n      RUSTC: rustc\n'),
      ).join('\n'),
    ).toContain('independent Rust selector')
    expect(
      toolchainInvariantErrors({
        ...files,
        '.github/actions/extra/action.yml':
          'runs:\n  using: composite\n  steps:\n    - run: node script.mjs\n',
      }).join('\n'),
    ).toContain('language tooling in a local action is unsupported')
  })

  it('rejects the former release-cache npx network fallback', () => {
    expect(
      toolchainInvariantErrors(
        mutate(ci, 'npx --no-install tauri', 'npx tauri'),
      ).join('\n'),
    ).toContain('npx must use --no-install')
  })

  it('preserves action SHA policy on a mutated checked workflow', () => {
    const changed = files[ci].replace(
      /actions\/checkout@[a-f0-9]{40}/,
      'actions/checkout@v4',
    )
    expect(changed).not.toBe(files[ci])
    expect(unpinnedActionErrors({ [ci]: changed }).join('\n')).toContain(
      'must be pinned to a full commit SHA',
    )
  })
})
