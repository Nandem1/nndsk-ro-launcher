import { afterEach, describe, expect, it, vi } from 'vitest'
import { mkdtemp, mkdir, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { createServer, mergeConfig, type ViteDevServer } from 'vite'
import viteConfig from '../../vite.config'

const fixtures: string[] = []
const servers: ViteDevServer[] = []

afterEach(async () => {
  for (const server of servers.splice(0)) await server.close()
  for (const fixture of fixtures.splice(0))
    await rm(fixture, { recursive: true, force: true })
})

async function fixture() {
  const scratch = await mkdtemp(join(tmpdir(), 'ro-vite-watch-'))
  fixtures.push(scratch)
  const root = join(scratch, 'project')
  for (const directory of [
    'src',
    'public',
    '.audit/prefix/dosdevices',
    'target/debug',
    'src-tauri/resources',
    'd7vk-spike-cache',
  ]) {
    await mkdir(join(root, directory), { recursive: true })
    await writeFile(join(root, directory, 'own-fixture.txt'), 'before')
  }
  const mapped = join(scratch, 'mapped-host')
  await mkdir(mapped)
  await writeFile(join(mapped, 'private-fixture.txt'), 'not frontend source')
  await symlink(mapped, join(root, '.audit/prefix/dosdevices/z:'))
  const server = await createServer(
    mergeConfig(viteConfig, {
      configFile: false,
      root,
      cacheDir: join(scratch, 'vite-cache'),
      logLevel: 'silent',
      server: { middlewareMode: true, hmr: false },
      optimizeDeps: { noDiscovery: true, include: [] },
    }),
  )
  servers.push(server)
  await vi.waitFor(
    () => {
      const watched = server.watcher.getWatched()
      expect(watched[join(root, 'src')]).toContain('own-fixture.txt')
      expect(watched[join(root, 'public')]).toContain('own-fixture.txt')
    },
    { timeout: 5000 },
  )
  return { root, mapped, server }
}

describe('frontend watcher scope', () => {
  it('does not traverse private evidence, Wine drive links or generated Rust trees', async () => {
    const { root, mapped, server } = await fixture()
    for (const directory of Object.keys(server.watcher.getWatched())) {
      const path = resolve(directory)
      expect(path.startsWith(mapped)).toBe(false)
      for (const ignored of [
        '.audit',
        'target',
        'src-tauri',
        'd7vk-spike-cache',
      ]) {
        const excluded = join(root, ignored)
        expect(path === excluded || path.startsWith(`${excluded}/`)).toBe(false)
      }
    }
  })

  it('still observes frontend and public asset changes', async () => {
    const { root, server } = await fixture()
    const changed = new Set<string>()
    server.watcher.on('change', (path) => changed.add(resolve(path)))
    const frontend = join(root, 'src/own-fixture.txt')
    const asset = join(root, 'public/own-fixture.txt')
    await writeFile(frontend, 'after')
    await writeFile(asset, 'after')
    await vi.waitFor(() => {
      expect(changed.has(frontend)).toBe(true)
      expect(changed.has(asset)).toBe(true)
    })
  })
})
