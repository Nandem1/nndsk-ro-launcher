import { describe, expect, it } from 'vitest'
import { createHash, generateKeyPairSync, sign } from 'node:crypto'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import {
  UPDATER_PLATFORMS,
  buildUpdaterManifest,
  signedVersion,
  verifyUpdaterSignature,
} from '../../scripts/updater-release.mjs'

const b64 = (data: Buffer | string) => Buffer.from(data).toString('base64')

function minisignFixture(keyId = Buffer.from('0123456789abcdef', 'hex')) {
  const { publicKey, privateKey } = generateKeyPairSync('ed25519')
  const rawPub = publicKey.export({ format: 'der', type: 'spki' }).subarray(-32)
  const pubFile = `untrusted comment: minisign public key\n${b64(
    Buffer.concat([Buffer.from('Ed'), keyId, rawPub]),
  )}\n`
  const signFile = (data: Buffer, trustedComment: string) => {
    const digest = createHash('blake2b512').update(data).digest()
    const signature = sign(null, digest, privateKey)
    const global = sign(
      null,
      Buffer.concat([signature, Buffer.from(trustedComment)]),
      privateKey,
    )
    return b64(
      `untrusted comment: signature from tauri secret key\n${b64(
        Buffer.concat([Buffer.from('ED'), keyId, signature]),
      )}\ntrusted comment: ${trustedComment}\n${b64(global)}\n`,
    )
  }
  return { pubkeyB64: b64(pubFile), signFile }
}

function writeArtifact(contents: string) {
  const path = join(mkdtempSync(join(tmpdir(), 'ro-sig-')), 'app.AppImage')
  writeFileSync(path, contents)
  return path
}

const COMMENT = 'timestamp:1\tfile:app.AppImage\tversion:1.2.3'

describe('verifyUpdaterSignature', () => {
  it('accepts a prehashed signature bound to the release version', async () => {
    const { pubkeyB64, signFile } = minisignFixture()
    const filePath = writeArtifact('appimage bytes')
    const signatureB64 = signFile(Buffer.from('appimage bytes'), COMMENT)
    await expect(
      verifyUpdaterSignature({
        filePath,
        signatureB64,
        pubkeyB64,
        expectedVersion: '1.2.3',
      }),
    ).resolves.toMatchObject({ version: '1.2.3' })
  })

  it('rejects a modified artifact', async () => {
    const { pubkeyB64, signFile } = minisignFixture()
    const signatureB64 = signFile(Buffer.from('appimage bytes'), COMMENT)
    await expect(
      verifyUpdaterSignature({
        filePath: writeArtifact('appimage bytez'),
        signatureB64,
        pubkeyB64,
        expectedVersion: '1.2.3',
      }),
    ).rejects.toThrow(/file signature is invalid/)
  })

  it('rejects a trusted comment rewritten to another version', async () => {
    const { pubkeyB64, signFile } = minisignFixture()
    const original = Buffer.from(
      signFile(Buffer.from('x'), COMMENT),
      'base64',
    ).toString()
    const forged = b64(original.replace('version:1.2.3', 'version:9.9.9'))
    await expect(
      verifyUpdaterSignature({
        filePath: writeArtifact('x'),
        signatureB64: forged,
        pubkeyB64,
        expectedVersion: '9.9.9',
      }),
    ).rejects.toThrow(/trusted comment signature is invalid/)
  })

  it('rejects a signature without a version binding', async () => {
    const { pubkeyB64, signFile } = minisignFixture()
    await expect(
      verifyUpdaterSignature({
        filePath: writeArtifact('x'),
        signatureB64: signFile(Buffer.from('x'), 'timestamp:1\tfile:x'),
        pubkeyB64,
        expectedVersion: '1.2.3',
      }),
    ).rejects.toThrow(/no version/)
  })

  it('rejects a signature from another key', async () => {
    const signer = minisignFixture(Buffer.from('1111111111111111', 'hex'))
    const verifier = minisignFixture(Buffer.from('2222222222222222', 'hex'))
    await expect(
      verifyUpdaterSignature({
        filePath: writeArtifact('x'),
        signatureB64: signer.signFile(Buffer.from('x'), COMMENT),
        pubkeyB64: verifier.pubkeyB64,
        expectedVersion: '1.2.3',
      }),
    ).rejects.toThrow(/key id/)
  })
})

describe('signedVersion', () => {
  it('matches the plugin field parsing', () => {
    expect(signedVersion(COMMENT)).toBe('1.2.3')
    expect(signedVersion('timestamp:1\tfile:app-version:2.zip')).toBeNull()
  })
})

describe('buildUpdaterManifest', () => {
  const url =
    'https://api.github.com/repos/Nandem1/nndsk-ro-launcher/releases/assets/42'

  it('writes the same platform keys tauri-action published', () => {
    const manifest = buildUpdaterManifest({
      version: '1.2.3',
      signatureB64: 'c2ln\n',
      url,
      pubDate: '2026-01-01T00:00:00.000Z',
    })
    expect(Object.keys(manifest.platforms)).toEqual(UPDATER_PLATFORMS)
    expect(manifest.platforms['linux-x86_64']).toEqual({
      signature: 'c2ln',
      url,
    })
    expect(manifest).toMatchObject({ version: '1.2.3', notes: '' })
  })

  it('refuses browser download urls that break for draft releases', () => {
    expect(() =>
      buildUpdaterManifest({
        version: '1.2.3',
        signatureB64: 'c2ln',
        url: 'https://github.com/o/r/releases/download/untagged-1/app.AppImage',
        pubDate: 'now',
      }),
    ).toThrow(/asset API url/)
  })
})
