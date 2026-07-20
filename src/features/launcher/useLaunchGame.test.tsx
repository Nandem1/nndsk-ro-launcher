// @vitest-environment jsdom

import { act, renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { DependencyStatus, ServerConfig } from '../../shared/types'
import { useServersStore } from '../servers/servers.store'
import { useSettingsStore } from '../settings/settings.store'
import { useLauncherStore } from './launcher.store'
import { useLaunchGame } from './useLaunchGame'
import { deferred } from '../../test/deferred'

const { checkMock, launchMock, setupMock } = vi.hoisted(() => ({
  checkMock: vi.fn(),
  launchMock: vi.fn(),
  setupMock: vi.fn(),
}))

vi.mock('../../shared/api', () => ({
  api: {
    checkDependencies: checkMock,
    setupPrefix: setupMock,
    launchGame: launchMock,
    stopGame: vi.fn(),
  },
}))

const server: ServerConfig = {
  id: 'sakura',
  name: 'SakuraRO',
  executablePath: '/games/sakura/ragexe.exe',
  prefixMode: 'isolated',
}

function readyStatus(): DependencyStatus {
  return {
    wine: true,
    winetricks: true,
    dxvk: true,
    prefixConfigured: true,
    audioOk: true,
    audioDriver: 'pulse',
    audioStack: 'PulseAudio',
    audioWarning: null,
    inputGroupOk: true,
    inputGroupWarning: null,
    uinputInputOk: true,
    uinputInputWarning: null,
    prefixOk: true,
    prefixWarning: null,
    dxvkOk: true,
    dxvkWarning: null,
    runnerKind: 'proton',
    runnerOk: true,
    runnerWarning: null,
    prefixPath: '/prefixes/sakura',
    prefixScope: 'isolated',
    prefixManaged: true,
    readyToLaunch: true,
    canSetup: true,
    canReset: true,
    checks: [],
  }
}

describe('useLaunchGame', () => {
  beforeEach(() => {
    checkMock.mockReset().mockResolvedValue(readyStatus())
    launchMock.mockReset().mockResolvedValue(undefined)
    setupMock.mockReset().mockResolvedValue(undefined)
    useServersStore.setState({
      servers: [server],
      selectedId: server.id,
      loading: false,
      error: null,
    })
    useSettingsStore.setState({ selectedRunner: '/opt/proton/proton' })
    useLauncherStore.setState({
      status: 'idle',
      setupProgress: null,
      error: null,
    })
  })

  it('coalesces a double launch before dependency preflight completes', async () => {
    const { result } = renderHook(() => useLaunchGame(server))
    let first!: Promise<void>
    let second!: Promise<void>
    act(() => {
      first = result.current.handleLaunch()
      second = result.current.handleLaunch()
    })
    await act(async () => Promise.all([first, second]))

    expect(checkMock).toHaveBeenCalledTimes(1)
    expect(launchMock).toHaveBeenCalledTimes(1)
    expect(setupMock).not.toHaveBeenCalled()
  })

  it('does not launch a stale argv snapshot after the server is edited', async () => {
    const pending = deferred<DependencyStatus>()
    checkMock.mockReturnValueOnce(pending.promise)
    const withArgs = {
      ...server,
      launch: { strategy: 'direct' as const, gameArgs: ['old'] },
    }
    useServersStore.setState({ servers: [withArgs] })
    const { result } = renderHook(() => useLaunchGame(withArgs))

    let launch!: Promise<void>
    act(() => {
      launch = result.current.handleLaunch()
    })
    useServersStore.setState({
      servers: [
        {
          ...withArgs,
          launch: { strategy: 'direct', gameArgs: ['new'] },
        },
      ],
    })
    pending.resolve(readyStatus())
    await act(async () => launch)

    expect(launchMock).not.toHaveBeenCalled()
    expect(useLauncherStore.getState().status).toBe('error')
  })

  it('does not continue setup after the effective runner changes', async () => {
    const pending = deferred<DependencyStatus>()
    checkMock.mockReturnValueOnce(pending.promise)
    const { result } = renderHook(() => useLaunchGame(server))

    let launch!: Promise<void>
    act(() => {
      launch = result.current.handleLaunch()
    })
    useSettingsStore.setState({ selectedRunner: '/opt/proton/other' })
    pending.resolve({ ...readyStatus(), readyToLaunch: false })
    await act(async () => launch)

    expect(setupMock).not.toHaveBeenCalled()
    expect(launchMock).not.toHaveBeenCalled()
  })
})
