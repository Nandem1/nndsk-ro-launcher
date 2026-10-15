// Structural release/CI invariants that must hold for a green release to mean something.
// Text-based on purpose: the repository has no YAML dependency and these rules only need job
// boundaries and literal lines.

const SIGNING_SECRETS = [
  'secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD',
  'secrets.TAURI_SIGNING_PRIVATE_KEY }}',
]

/** Splits the `jobs:` section of a workflow into `{ name: text }`. */
export function workflowJobs(yaml) {
  const lines = yaml.split('\n')
  const start = lines.findIndex((line) => /^jobs:\s*$/.test(line))
  if (start < 0) return {}
  const jobs = {}
  let current = null
  for (const line of lines.slice(start + 1)) {
    const header = line.match(/^ {2}([A-Za-z0-9_-]+):\s*$/)
    if (header) {
      current = header[1]
      jobs[current] = ''
      continue
    }
    if (/^\S/.test(line)) break
    if (current) jobs[current] += `${line}\n`
  }
  return jobs
}

/** Returns the step (from its `- ` line to the next one) that contains `needle`. */
function stepContaining(jobText, needle) {
  const steps = jobText.split(/\n(?= {6}- )/)
  return steps.find((step) => step.includes(needle)) ?? null
}

/** Every remote action must be pinned to a full commit SHA. */
export function unpinnedActionErrors(files) {
  const errors = []
  for (const [path, text] of Object.entries(files)) {
    for (const match of text.matchAll(/^\s*-?\s*uses:\s*(\S+)/gm)) {
      const ref = match[1]
      if (ref.startsWith('./')) continue
      if (!/^[^@\s]+@[0-9a-f]{40}$/.test(ref)) {
        errors.push(
          `${path}: action ${ref} must be pinned to a full commit SHA`,
        )
      }
    }
  }
  return errors
}

/** CI must not hide a failing command or a skipped Yama assertion. */
export function hiddenFailureErrors(files) {
  const errors = []
  for (const [path, text] of Object.entries(files)) {
    if (/^\s*continue-on-error:/m.test(text)) {
      errors.push(`${path}: continue-on-error hides failing commands`)
    }
    if (/\|\|\s*true\b/.test(text)) {
      errors.push(`${path}: "|| true" hides failing commands`)
    }
    if (text.includes('RO_YAMA_ALLOW_UNVALIDATED')) {
      errors.push(
        `${path}: workflows must not opt out of the Yama=1 assertions`,
      )
    }
  }
  return errors
}

/** Every PR must produce the stable dependency checks before merge (F-13). */
export function dependencyAuditWorkflowErrors(yaml) {
  const errors = hiddenFailureErrors({ 'dependency-audit.yml': yaml })
  // Support only the literal, empty PR mapping. Reject all nested/inline options rather than
  // partially parsing YAML and overlooking an alias, merge key or future narrowing option.
  const source = yaml.replace(/^[ \t]*#.*$/gm, '')
  const triggers = source.match(/^on:\n((?:^[ \t]+[^\n]*\n|^\n)*)/m)?.[1] ?? ''
  const pr = triggers.match(
    /^ {2}pull_request:([^\n]*)\n((?:^[ \t]{3}[^\n]*\n|^\n)*)/m,
  )
  if (
    !pr ||
    pr[1].trim() !== '' ||
    pr[2]
      .split('\n')
      .some((line) => line.trim() && !line.trim().startsWith('#')) ||
    [...triggers.matchAll(/^ {2}pull_request:/gm)].length !== 1
  )
    errors.push(
      'dependency-audit.yml must audit all pull requests with an unconditional pull_request trigger',
    )
  const jobs = workflowJobs(yaml)
  for (const [job, name, mode] of [
    ['npm', 'npm-dependency-audit', 'npm'],
    ['rustsec', 'rustsec-dependency-audit', 'cargo'],
  ]) {
    if (!jobs[job]?.includes(`    name: ${name}\n`))
      errors.push(`dependency-audit.yml must keep stable check name ${name}`)
    if (
      !jobs[job]?.includes(
        `      - run: node scripts/dependency-audit-policy.mjs ${mode}\n`,
      )
    )
      errors.push(
        `dependency-audit.yml ${job} must run the shared ${mode} policy`,
      )
    if (
      /^\s+(?:-\s+)?if:/m.test(jobs[job] ?? '') ||
      /\bset \+e\b/m.test(jobs[job] ?? '')
    )
      errors.push(
        `dependency-audit.yml ${job} must not skip or suppress the audit`,
      )
  }
  return errors
}

/** Signing isolation, tested-artifact flow and draft publication in release.yml. */
export function releaseWorkflowErrors(yaml) {
  const errors = []
  const jobs = workflowJobs(yaml)
  for (const name of ['build', 'sign', 'publish']) {
    if (!jobs[name]) errors.push(`release.yml must define a ${name} job`)
  }
  if (errors.length > 0) return errors

  if (!/^permissions:\s*\{\}\s*$/m.test(yaml)) {
    errors.push('release.yml must default to permissions: {} and grant per job')
  }
  if (yaml.includes('tauri-apps/tauri-action')) {
    errors.push(
      'release.yml must not use tauri-action: it signs inside the build step',
    )
  }
  for (const [name, text] of Object.entries(jobs)) {
    if (name !== 'publish' && /contents:\s*write/.test(text)) {
      errors.push(`release.yml job ${name} must not have contents: write`)
    }
    if (name !== 'sign' && /TAURI_SIGNING_PRIVATE_KEY/.test(text)) {
      errors.push(`release.yml job ${name} must not reference the signing key`)
    }
  }
  for (const secret of SIGNING_SECRETS) {
    const count = yaml.split(secret).length - 1
    if (count !== 1) {
      errors.push(
        `release.yml must reference ${secret.replace(' }}', '')} exactly once (found ${count})`,
      )
    }
  }
  const signStep = stepContaining(
    jobs.sign,
    'secrets.TAURI_SIGNING_PRIVATE_KEY }}',
  )
  if (!signStep || !signStep.includes('tauri signer sign --app-version')) {
    errors.push(
      'the signing key must only reach the tauri signer sign --app-version step',
    )
  } else if (/npm (ci|install|run)|cargo |npx /.test(signStep)) {
    errors.push('the signing step must not run npm, npx or cargo')
  }
  if (!/npm ci --ignore-scripts/.test(jobs.sign)) {
    errors.push(
      'the sign job must install the CLI with npm ci --ignore-scripts',
    )
  }
  if (!jobs.build.includes('createUpdaterArtifacts":false')) {
    errors.push(
      'the build job must produce an unsigned AppImage (createUpdaterArtifacts false)',
    )
  }
  if (!jobs.build.includes('scripts/smoke-release-artifact.sh')) {
    errors.push('the build job must smoke-test the AppImage it uploads')
  }
  if (
    !jobs.build.includes('dependency-audit-policy.mjs npm') ||
    !jobs.build.includes('dependency-audit-policy.mjs cargo')
  ) {
    errors.push('the build job must run the npm and RustSec audit policy')
  }
  for (const name of ['sign', 'publish']) {
    if (!jobs[name].includes('needs.build.outputs.sha256')) {
      errors.push(`the ${name} job must re-check the tested AppImage SHA-256`)
    }
    if (!jobs[name].includes('updater-release.mjs verify') && name === 'sign') {
      errors.push(
        'the sign job must verify the signature against the embedded pubkey',
      )
    }
  }
  if (!jobs.publish.includes('updater-release.mjs verify')) {
    errors.push('the publish job must verify the signature before publishing')
  }
  if (!jobs.publish.includes('updater-release.mjs manifest')) {
    errors.push(
      'the publish job must write latest.json with updater-release.mjs manifest',
    )
  }
  if (!jobs.publish.includes('--draft')) {
    errors.push('the publish job must create a draft release')
  }
  return errors
}
