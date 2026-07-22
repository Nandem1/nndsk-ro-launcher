import { describe, expect, it } from 'vitest'
import type { GameClientSnapshot } from '../../shared/types'
import { isSoleRunningClientForServer } from './launcher.store'

function client(clientId: string, serverId = 'sakura'): GameClientSnapshot {
  return {
    clientId,
    serverId,
    serverName: serverId,
    status: 'running',
    pid: 42,
  }
}

describe('multi-client tool availability', () => {
  it('enables tools only for the sole running client and matching server', () => {
    expect(
      isSoleRunningClientForServer({ clients: [client('one')] }, 'sakura'),
    ).toBe(true)
    expect(
      isSoleRunningClientForServer({ clients: [client('one')] }, 'other'),
    ).toBe(false)
  })

  it('disables tools while launching or when multiple clients exist', () => {
    expect(
      isSoleRunningClientForServer(
        {
          clients: [{ ...client('one'), status: 'launching', pid: null }],
        },
        'sakura',
      ),
    ).toBe(false)
    expect(
      isSoleRunningClientForServer(
        { clients: [client('one'), client('two')] },
        'sakura',
      ),
    ).toBe(false)
  })
})
