#!/usr/bin/env node
// Release-side mirror of tauri-plugin-updater's signature check, plus the latest.json writer.
//
// The signing job only signs. The publish job, which never sees the private key, uses this script
// to prove that the detached signature verifies against the pubkey embedded in tauri.conf.json and
// that its trusted comment binds the exact release version (`requireSignedVersion`). Only then is
// latest.json written. The checks follow tauri-plugin-updater 2.12 `verify_signature` and
// minisign-verify: Ed25519 over BLAKE2b-512(file) for prehashed `ED` signatures (or the raw file
// for legacy `Ed`), then a global Ed25519 signature over `signature || trusted_comment`.
import { createHash, createPublicKey, verify } from 'node:crypto'
import { createReadStream, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const ED25519_SPKI_PREFIX = Buffer.from('302a300506032b6570032100', 'hex')
// tauri-plugin-updater looks for `{os}-{arch}-{installer}` first and then `{os}-{arch}`.
export const UPDATER_PLATFORMS = ['linux-x86_64', 'linux-x86_64-appimage']

function fail(message) {
  throw new Error(message)
}

function decodeBase64Text(label, value) {
  const text = Buffer.from(value.trim(), 'base64').toString('utf8')
  if (text.length === 0) fail(`${label} is not base64 text`)
  return text
}

export function parsePublicKey(pubkeyB64) {
  const lines = decodeBase64Text('pubkey', pubkeyB64).split('\n')
  const bin = Buffer.from((lines[1] ?? '').trim(), 'base64')
  if (bin.length !== 42 || bin.subarray(0, 2).toString('latin1') !== 'Ed') {
    fail('pubkey is not a minisign Ed25519 public key')
  }
  return {
    keyId: bin.subarray(2, 10),
    key: createPublicKey({
      key: Buffer.concat([ED25519_SPKI_PREFIX, bin.subarray(10)]),
      format: 'der',
      type: 'spki',
    }),
  }
}

export function parseSignature(signatureB64) {
  const lines = decodeBase64Text('signature', signatureB64).split('\n')
  if (!lines[0]?.startsWith('untrusted comment: '))
    fail('signature has no untrusted comment line')
  const bin = Buffer.from((lines[1] ?? '').trim(), 'base64')
  if (bin.length !== 74) fail('signature line has the wrong length')
  const algorithm = bin.subarray(0, 2).toString('latin1')
  if (algorithm !== 'Ed' && algorithm !== 'ED')
    fail(`unsupported signature algorithm ${algorithm}`)
  const trustedLine = lines[2] ?? ''
  if (!trustedLine.startsWith('trusted comment: '))
    fail('signature has no trusted comment line')
  const globalSignature = Buffer.from((lines[3] ?? '').trim(), 'base64')
  if (globalSignature.length !== 64)
    fail('global signature has the wrong length')
  return {
    prehashed: algorithm === 'ED',
    keyId: bin.subarray(2, 10),
    signature: bin.subarray(10),
    trustedComment: trustedLine.slice('trusted comment: '.length),
    globalSignature,
  }
}

/** Same parsing as tauri-plugin-updater `signed_version`: tab separated `key:value` fields. */
export function signedVersion(trustedComment) {
  for (const field of trustedComment.split('\t')) {
    if (field.startsWith('version:')) return field.slice('version:'.length)
  }
  return null
}

async function blake2b512File(path) {
  const hash = createHash('blake2b512')
  for await (const chunk of createReadStream(path)) hash.update(chunk)
  return hash.digest()
}

/**
 * Verifies `filePath` against a Tauri `.sig` (base64 minisign) and requires the trusted comment
 * to carry `version:<expectedVersion>`. Throws on any mismatch.
 */
export async function verifyUpdaterSignature({
  filePath,
  signatureB64,
  pubkeyB64,
  expectedVersion,
}) {
  const pub = parsePublicKey(pubkeyB64)
  const sig = parseSignature(signatureB64)
  if (!pub.keyId.equals(sig.keyId))
    fail('signature key id does not match the updater pubkey')
  const message = sig.prehashed
    ? await blake2b512File(filePath)
    : readFileSync(filePath)
  if (!verify(null, message, pub.key, sig.signature))
    fail('file signature is invalid')
  const global = Buffer.concat([
    sig.signature,
    Buffer.from(sig.trustedComment, 'utf8'),
  ])
  if (!verify(null, global, pub.key, sig.globalSignature)) {
    fail('trusted comment signature is invalid')
  }
  const version = signedVersion(sig.trustedComment)
  if (version === null)
    fail('trusted comment has no version: field (requireSignedVersion)')
  if (version.replace(/^v/, '') !== expectedVersion.replace(/^v/, '')) {
    fail(`signature binds version ${version}, release is ${expectedVersion}`)
  }
  return { trustedComment: sig.trustedComment, version }
}

export function buildUpdaterManifest({ version, signatureB64, url, pubDate }) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) fail(`invalid version ${version}`)
  if (
    !/^https:\/\/api\.github\.com\/repos\/[^/]+\/[^/]+\/releases\/assets\/\d+$/.test(
      url,
    )
  ) {
    fail(`updater url must be a GitHub release asset API url: ${url}`)
  }
  const entry = { signature: signatureB64.trim(), url }
  return {
    version,
    notes: '',
    pub_date: pubDate,
    platforms: Object.fromEntries(
      UPDATER_PLATFORMS.map((p) => [p, { ...entry }]),
    ),
  }
}

function configuredPubkey() {
  const conf = JSON.parse(
    readFileSync(join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'),
  )
  const pubkey = conf.plugins?.updater?.pubkey
  if (typeof pubkey !== 'string' || pubkey.trim() === '')
    fail('tauri.conf.json has no pubkey')
  return pubkey
}

function option(args, name) {
  const index = args.indexOf(name)
  if (index < 0 || index + 1 >= args.length) fail(`missing ${name}`)
  return args[index + 1]
}

async function main(args) {
  const [command] = args
  const filePath = option(args, '--file')
  const signatureB64 = readFileSync(option(args, '--sig'), 'utf8')
  const expectedVersion = option(args, '--version')
  const pubkeyB64 = args.includes('--pubkey')
    ? option(args, '--pubkey')
    : configuredPubkey()
  const verified = await verifyUpdaterSignature({
    filePath,
    signatureB64,
    pubkeyB64,
    expectedVersion,
  })
  console.log(
    `updater signature ok: ${JSON.stringify(verified.trustedComment)}`,
  )
  if (command === 'verify') return
  if (command !== 'manifest')
    fail(
      'usage: updater-release.mjs verify|manifest --file F --sig S --version X',
    )
  const manifest = buildUpdaterManifest({
    version: expectedVersion,
    signatureB64,
    url: option(args, '--url'),
    pubDate: new Date().toISOString(),
  })
  writeFileSync(option(args, '--out'), `${JSON.stringify(manifest, null, 2)}\n`)
  console.log(`latest.json written for ${expectedVersion}`)
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch((error) => {
    console.error(`updater-release: ${error.message}`)
    process.exit(1)
  })
}
