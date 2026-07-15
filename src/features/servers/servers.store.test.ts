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
})
