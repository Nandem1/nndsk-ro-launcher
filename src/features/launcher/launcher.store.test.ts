import { afterEach, describe, expect, it } from 'vitest'
import type { GameClientSnapshot } from '../../shared/types'
import {
  clearSettledLaunchFeedback,
  isSoleRunningClientForServer,
  useLauncherStore,
} from './launcher.store'

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

describe('clearSettledLaunchFeedback', () => {
  afterEach(() => {
    useLauncherStore.setState({
      status: 'idle',
      error: null,
      setupProgress: null,
    })
  })

  it('returns a cancelled launch to idle', () => {
    useLauncherStore.setState({
      status: 'error',
      error: 'El lanzamiento fue cancelado por el usuario',
    })
    clearSettledLaunchFeedback()
    expect(useLauncherStore.getState()).toMatchObject({
      status: 'idle',
      error: null,
    })
  })

  it('leaves an in-flight launch running', () => {
    useLauncherStore.setState({ status: 'launching', error: null })
    clearSettledLaunchFeedback()
    expect(useLauncherStore.getState().status).toBe('launching')
  })
})
