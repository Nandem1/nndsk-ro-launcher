#!/usr/bin/env node
// Build-job environment gate for release.yml.
//
// The AppImage is compiled, bundled and smoke-tested without the updater signing key; only the
// separate sign job sees it. This gate therefore requires the public Discord Application ID that
// is embedded at compile time and fails if signing material leaked into the build environment.

function fail(message) {
  console.error(`require-release-env: ${message}`)
  process.exit(1)
}

const discord = process.env.RO_LAUNCHER_DISCORD_APPLICATION_ID ?? ''
if (!/^[0-9]+$/.test(discord)) {
  fail(
    'RO_LAUNCHER_DISCORD_APPLICATION_ID must be a non-empty numeric Application ID',
  )
}

for (const name of [
  'TAURI_SIGNING_PRIVATE_KEY',
  'TAURI_SIGNING_PRIVATE_KEY_PATH',
  'TAURI_SIGNING_PRIVATE_KEY_PASSWORD',
]) {
  if (name in process.env) {
    fail(
      `${name} must not be visible to the build job; only the sign job may receive it`,
    )
  }
}

console.log('require-release-env: ok')
