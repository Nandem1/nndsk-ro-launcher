import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../../shared/api'
import type { ServerConfig } from '../../shared/types'
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

  it('publishes an update only after persistence succeeds', async () => {
    const save = vi.spyOn(api, 'saveServers').mockResolvedValue()

    await useServersStore
      .getState()
      .updateServer(server.id, { name: 'Updated' })

    expect(save).toHaveBeenCalledWith([{ ...server, name: 'Updated' }])
    expect(useServersStore.getState().servers[0].name).toBe('Updated')
  })

  it('keeps the previous value when persistence fails', async () => {
    vi.spyOn(api, 'saveServers').mockRejectedValue(new Error('disk full'))

    await useServersStore
      .getState()
      .updateServer(server.id, { name: 'Updated' })

    expect(useServersStore.getState().servers[0].name).toBe(server.name)
    expect(useServersStore.getState().error).toBe('disk full')
  })
})
