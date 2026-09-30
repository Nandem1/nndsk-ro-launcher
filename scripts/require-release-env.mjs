#!/usr/bin/env node

function fail(message) {
  console.error(`require-release-env: ${message}`)
  process.exit(1)
}

const discord = process.env.RO_LAUNCHER_DISCORD_APPLICATION_ID ?? ''
if (!/^[0-9]+$/.test(discord)) {
  fail('RO_LAUNCHER_DISCORD_APPLICATION_ID must be a non-empty numeric Application ID')
}

const key = process.env.TAURI_SIGNING_PRIVATE_KEY ?? ''
if (key.trim() === '') {
  fail('TAURI_SIGNING_PRIVATE_KEY must be non-empty')
}

// Password may be empty when the key is unencrypted.
void process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD

console.log('require-release-env: ok')
