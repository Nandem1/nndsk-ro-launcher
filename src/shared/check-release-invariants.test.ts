import { describe, expect, it } from 'vitest'
import { spawnSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { sidecarOrderError } from '../../scripts/release-sidecar-order.mjs'
import { qualityCiTriggerError } from '../../scripts/quality-ci-trigger.mjs'
import {
  dependencyAuditWorkflowErrors,
  hiddenFailureErrors,
  releaseWorkflowErrors,
  unpinnedActionErrors,
  workflowJobs,
} from '../../scripts/release-workflow-policy.mjs'

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
      - run: npx tauri build --bundles appimage
`
    expect(sidecarOrderError(broken)).toMatch(/debug sidecars/)
  })

  it('accepts the current release.yml order', () => {
    const yaml = readFileSync(
      join(root, '.github/workflows/release.yml'),
      'utf8',
    )
    expect(sidecarOrderError(yaml)).toBeNull()
  })
})

describe('releaseWorkflowErrors', () => {
  const release = readFileSync(
    join(root, '.github/workflows/release.yml'),
    'utf8',
  )

  it('accepts the current build, sign and publish split', () => {
    expect(releaseWorkflowErrors(release)).toEqual([])
    expect(Object.keys(workflowJobs(release))).toEqual([
      'release-config',
      'build',
      'sign',
      'publish',
    ])
  })

  it('rejects the signing key leaking into the build job', () => {
    const leaked = release.replace(
      "          DISCORD_APPLICATION_ID: ''\n",
      "          DISCORD_APPLICATION_ID: ''\n          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}\n",
    )
    expect(leaked).not.toBe(release)
    expect(releaseWorkflowErrors(leaked).join('\n')).toMatch(
      /job build must not reference the signing key/,
    )
  })

  it('rejects package code running beside the private key', () => {
    const noisy = release.replace(
      './node_modules/.bin/tauri signer sign',
      'npm run build && ./node_modules/.bin/tauri signer sign',
    )
    expect(noisy).not.toBe(release)
    expect(releaseWorkflowErrors(noisy).join('\n')).toMatch(/must not run npm/)
  })

  it('rejects returning to in-build signing through tauri-action', () => {
    const legacy = `${release}\n      - uses: tauri-apps/tauri-action@1deb371b0cd8bd54025b384f1cd735e725c4060f\n`
    expect(releaseWorkflowErrors(legacy).join('\n')).toMatch(/tauri-action/)
  })

  it('rejects publishing without the smoke test or the hash re-check', () => {
    const untested = release
      .replace('scripts/smoke-release-artifact.sh', 'true')
      .replaceAll('needs.build.outputs.sha256', 'needs.build.outputs.other')
    const errors = releaseWorkflowErrors(untested).join('\n')
    expect(errors).toMatch(/smoke-test/)
    expect(errors).toMatch(/re-check the tested AppImage SHA-256/)
  })
})

describe('workflow hygiene', () => {
  it('rejects mutable action tags and hidden failures', () => {
    const yaml = `
jobs:
  audit:
    continue-on-error: true
    steps:
      - uses: actions/checkout@v4
      - uses: ./.github/actions/linux-build-deps
      - run: cargo audit || true
      - run: RO_YAMA_ALLOW_UNVALIDATED=1 cargo test
`
    expect(unpinnedActionErrors({ 'x.yml': yaml })).toEqual([
      'x.yml: action actions/checkout@v4 must be pinned to a full commit SHA',
    ])
    expect(hiddenFailureErrors({ 'x.yml': yaml })).toHaveLength(3)
  })
})

describe('dependencyAuditWorkflowErrors', () => {
  const audit = readFileSync(
    join(root, '.github/workflows/dependency-audit.yml'),
    'utf8',
  )

  it('runs shared audits on every PR with stable check names', () => {
    expect(dependencyAuditWorkflowErrors(audit)).toEqual([])
  })

  it('rejects removing the PR trigger', () => {
    expect(
      dependencyAuditWorkflowErrors(
        audit.replace('  pull_request:', '  push:'),
      ).join('\n'),
    ).toMatch(/pull requests/)
  })

  it.each([
    ['paths', 'Cargo.lock'],
    ['paths-ignore', 'Cargo.lock'],
    ['branches', 'main'],
    ['branches-ignore', 'main'],
    ['types', 'opened'],
  ])('rejects pull_request.%s filters', (filter, value) => {
    const changed = audit.replace(
      '  pull_request:\n',
      `  pull_request:\n    ${filter}: ['${value}']\n`,
    )
    expect(dependencyAuditWorkflowErrors(changed).join('\n')).toMatch(
      /unconditional pull_request/,
    )
  })

  it.each([
    '  pull_request: { paths: [Cargo.lock] }\n',
    '  pull_request: *filtered\n',
    '  pull_request:\n    <<: *filtered\n',
    '  pull_request:\n  pull_request:\n    paths: [Cargo.lock]\n',
    '  pull_request:\n   paths: [Cargo.lock]\n',
    '  pull_request:\n# event filter\n    paths: [Cargo.lock]\n',
  ])('rejects inline, aliased or duplicate PR options: %s', (trigger) => {
    expect(
      dependencyAuditWorkflowErrors(
        audit.replace('  pull_request:\n', trigger),
      ).join('\n'),
    ).toMatch(/unconditional pull_request/)
  })

  it.each([
    ['npm', 'npm-dependency-audit', 'npm'],
    ['rustsec', 'rustsec-dependency-audit', 'cargo'],
  ])(
    'rejects skips, suppression and renamed checks in %s',
    (job, name, mode) => {
      const command = `      - run: node scripts/dependency-audit-policy.mjs ${mode}\n`
      for (const [before, after] of [
        [`name: ${name}`, 'name: optional'],
        [`  ${job}:\n`, `  ${job}:\n    if: false\n`],
        [command, `${command}        if: false\n`],
        [
          command,
          `      - if: false\n        run: node scripts/dependency-audit-policy.mjs ${mode}\n`,
        ],
        [`  ${job}:\n`, `  ${job}:\n    continue-on-error: true\n`],
        [command, `${command}        continue-on-error: true\n`],
        [command, command.replace(`${mode}\n`, `${mode} || true\n`)],
        [
          command,
          `      - run: |\n          set +e\n          node scripts/dependency-audit-policy.mjs ${mode}\n`,
        ],
        [command, '      - run: true\n'],
        [command, `      - run: set +e\n${command}`],
      ]) {
        const changed = audit.replace(before, after)
        expect(changed).not.toBe(audit)
        expect(dependencyAuditWorkflowErrors(changed).length).toBeGreaterThan(0)
      }
    },
  )
})

describe('qualityCiTriggerError', () => {
  it('rejects a bare push trigger that also fires on tags', () => {
    const unrestricted = `
on:
  push:
  pull_request:
`
    expect(qualityCiTriggerError(unrestricted)).toMatch(/tag pushes/)
  })

  it('accepts the current ci.yml trigger', () => {
    const yaml = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
    expect(qualityCiTriggerError(yaml)).toBeNull()
    expect(yaml).toMatch(/branches:\s*\[main\]/)
  })
})

describe('undecorated window chrome', () => {
  it('keeps decorations false and guest window controls', () => {
    const tauriConf = JSON.parse(
      readFileSync(join(root, 'src-tauri/tauri.conf.json'), 'utf8'),
    )
    const capabilities = readFileSync(
      join(root, 'src-tauri/capabilities/default.json'),
      'utf8',
    )
    expect(tauriConf.app.windows[0].decorations).toBe(false)
    expect(tauriConf.app.windows[0].title).toBe('RO-Launcher')
    expect(capabilities).toMatch(/core:window:allow-close/)
    expect(capabilities).toMatch(/core:window:allow-minimize/)
    expect(capabilities).toMatch(/core:window:allow-start-dragging/)
    const webviewRs = readFileSync(
      join(root, 'src-tauri/src/utils/webview.rs'),
      'utf8',
    )
    expect(webviewRs).toMatch(/set_titlebar\(None/)
  })
})
