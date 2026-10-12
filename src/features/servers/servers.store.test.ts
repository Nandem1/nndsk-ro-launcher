import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../../shared/api'
import type { ServerConfig } from '../../shared/types'
import { deferred } from '../../test/deferred'
import { useServersStore } from './servers.store'

const server: ServerConfig = {
  id: 'server-1',
  name: 'Test RO',
  executablePath: '/games/test/Ragexe.exe',
}

describe('servers store', () => {
  beforeEach(() => {
    useServersStore.setState({
      servers: [server],
      selectedId: server.id,
      loading: false,
      error: null,
    })
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('publishes an update immediately and confirms persistence', async () => {
    const pending = deferred<void>()
    const save = vi.spyOn(api, 'saveServers').mockReturnValue(pending.promise)

    const update = useServersStore
      .getState()
      .updateServer(server.id, { name: 'Updated' })

    expect(save).toHaveBeenCalledWith([{ ...server, name: 'Updated' }])
    expect(useServersStore.getState().servers[0].name).toBe('Updated')
    pending.resolve()
    await expect(update).resolves.toMatchObject({ name: 'Updated' })
  })

  it('keeps the optimistic value and exposes persistence failures', async () => {
    vi.spyOn(api, 'saveServers').mockRejectedValue(new Error('disk full'))

    await useServersStore
      .getState()
      .updateServer(server.id, { name: 'Updated' })

    expect(useServersStore.getState().servers[0].name).toBe('Updated')
    expect(useServersStore.getState().error).toBe('disk full')
  })

  it('coalesces rapid functional updates without losing fields', async () => {
    const first = deferred<void>()
    const save = vi
      .spyOn(api, 'saveServers')
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce()

    const rename = useServersStore
      .getState()
      .updateServer(server.id, (current) => ({
        ...current,
        name: 'Updated',
      }))
    const runner = useServersStore
      .getState()
      .updateServer(server.id, (current) => ({
        ...current,
        runner: '/usr/bin/wine',
      }))

    expect(useServersStore.getState().servers[0]).toMatchObject({
      name: 'Updated',
      runner: '/usr/bin/wine',
    })
    expect(save).toHaveBeenCalledTimes(1)

    first.resolve()
    await Promise.all([rename, runner])

    expect(save).toHaveBeenCalledTimes(2)
    expect(save.mock.calls[1][0][0]).toMatchObject({
      name: 'Updated',
      runner: '/usr/bin/wine',
    })
  })

  it('discards an older server read and preserves the current selection', async () => {
    const older = deferred<ServerConfig[]>()
    vi.spyOn(api, 'listServers')
      .mockReturnValueOnce(older.promise)
      .mockResolvedValueOnce([server, { ...server, id: 'second' }])
    const first = useServersStore.getState().loadServers()
    useServersStore.getState().selectServer('second')
    await useServersStore.getState().loadServers()
    older.resolve([])
    await first
    expect(useServersStore.getState()).toMatchObject({
      selectedId: 'second',
      loading: false,
    })
    expect(useServersStore.getState().servers).toHaveLength(2)
  })

  it('discards a disk snapshot captured before a local edit', async () => {
    const pending = deferred<ServerConfig[]>()
    vi.spyOn(api, 'listServers').mockReturnValueOnce(pending.promise)
    vi.spyOn(api, 'saveServers').mockResolvedValue()
    const read = useServersStore.getState().loadServers()
    await useServersStore.getState().updateServer(server.id, { name: 'New' })
    pending.resolve([server])
    await read
    expect(useServersStore.getState().servers[0].name).toBe('New')
  })

  it('merges edits made while an installation is registered without duplicating its server', async () => {
    const pending = deferred<ServerConfig>()
    const installed = {
      ...server,
      id: 'installed-1',
      name: 'Installed',
      installation: {
        installationId: 'install-1',
        prefixPath: '/prefix/installed',
        prefixFingerprint: 'a'.repeat(64),
      },
    }
    const save = vi.spyOn(api, 'saveServers').mockResolvedValue()
    const confirm = vi
      .spyOn(api, 'confirmInstallation')
      .mockReturnValue(pending.promise)
    const registration = useServersStore
      .getState()
      .registerInstallation('install-1', installed.executablePath, null)
    await vi.waitFor(() => expect(confirm).toHaveBeenCalledOnce())
    const edit = useServersStore
      .getState()
      .updateServer(server.id, { name: 'Edited during installation' })
    // No snapshot may race the backend's insert while confirmation is pending.
    expect(save).toHaveBeenCalledTimes(1)
    pending.resolve(installed)
    await Promise.all([registration, edit])
    expect(useServersStore.getState().servers).toEqual([
      { ...server, name: 'Edited during installation' },
      installed,
    ])
    await useServersStore
      .getState()
      .updateServer(installed.id, { name: 'Customized' })
    await useServersStore
      .getState()
      .registerInstallation('install-1', installed.executablePath, null)
    expect(useServersStore.getState().servers).toHaveLength(2)
    expect(useServersStore.getState().servers[1].name).toBe('Customized')
    expect(save.mock.lastCall?.[0]).toEqual(useServersStore.getState().servers)
  })

  it('preserves a durable installation insert if completing its draft fails', async () => {
    const pending = deferred<ServerConfig>()
    const installed: ServerConfig = {
      ...server,
      id: 'installed-partial',
      installation: {
        installationId: 'install-partial',
        prefixPath: '/prefix/partial',
        prefixFingerprint: 'b'.repeat(64),
      },
    }
    const save = vi.spyOn(api, 'saveServers').mockResolvedValue()
    const confirm = vi
      .spyOn(api, 'confirmInstallation')
      .mockReturnValue(pending.promise)
    vi.spyOn(api, 'listServers').mockResolvedValue([server, installed])
    const registration = useServersStore
      .getState()
      .registerInstallation('install-partial', installed.executablePath, null)
    const failed = expect(registration).rejects.toThrow('draft write failed')
    await vi.waitFor(() => expect(confirm).toHaveBeenCalledOnce())
    const edit = useServersStore
      .getState()
      .updateServer(server.id, { name: 'Concurrent edit' })
    pending.reject(new Error('draft write failed'))
    await Promise.all([failed, edit])
    expect(save.mock.lastCall?.[0]).toEqual([
      { ...server, name: 'Concurrent edit' },
      installed,
    ])
  })
})
