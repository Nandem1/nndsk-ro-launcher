// @vitest-environment jsdom

import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
} from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type {
  DependencyStatus,
  GameClientSnapshot,
  RunnerInfo,
  ServerConfig,
} from '../../shared/types'
import { useServersStore } from '../servers/servers.store'
import { useSettingsStore } from '../settings/settings.store'
import { useLauncherStore } from './launcher.store'
import { useLaunchGame } from './useLaunchGame'
import { deferred } from '../../test/deferred'
import { LaunchButton } from './LaunchButton'
import { useSelectedRuntimeStatus } from '../settings/useSelectedRuntimeStatus'
import { MANAGED_RUNTIME_ID } from '../../shared/constants'

const {
  checkMock,
  launchMock,
  listClientsMock,
  setupMock,
  runnersMock,
  serversMock,
} = vi.hoisted(() => ({
  checkMock: vi.fn(),
  launchMock: vi.fn(),
  listClientsMock: vi.fn(),
  setupMock: vi.fn(),
  runnersMock: vi.fn(),
  serversMock: vi.fn(),
}))

vi.mock('../../shared/api', () => ({
  api: {
    checkDependencies: checkMock,
    setupPrefix: setupMock,
    launchGame: launchMock,
    listGameClients: listClientsMock,
    stopGame: vi.fn(),
    loadSettings: vi.fn(async () => ({ defaultRunner: '/opt/proton/proton' })),
    listRunners: runnersMock,
    listServers: serversMock,
    saveSettings: vi.fn(),
  },
}))

let runtimeClients: GameClientSnapshot[] = []

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

function Launcher() {
  useSelectedRuntimeStatus()
  return <LaunchButton />
}

describe('useLaunchGame', () => {
  beforeEach(() => {
    checkMock.mockReset().mockResolvedValue(readyStatus())
    runtimeClients = []
    launchMock.mockReset().mockImplementation(async (clientId: string) => {
      const client: GameClientSnapshot = {
        clientId,
        serverId: server.id,
        serverName: server.name,
        status: 'running',
        pid: 100 + runtimeClients.length,
      }
      runtimeClients.push(client)
      return client
    })
    listClientsMock
      .mockReset()
      .mockImplementation(async () => [...runtimeClients])
    setupMock.mockReset().mockResolvedValue(undefined)
    runnersMock
      .mockReset()
      .mockResolvedValue([
        { id: MANAGED_RUNTIME_ID, name: 'Proton', path: '/opt/proton/proton' },
      ])
    serversMock.mockReset().mockResolvedValue([server])
    useServersStore.setState({
      servers: [server],
      selectedId: server.id,
      loading: false,
      error: null,
    })
    useSettingsStore.setState({
      selectedRunner: '/opt/proton/proton',
      loading: false,
      savingRunner: false,
      advancedStatus: null,
      advancedStatusKey: null,
      advancedStatusError: null,
    })
    useLauncherStore.setState({
      status: 'idle',
      clients: [],
      clientsRevision: 0,
      closedClientIds: [],
      setupProgress: null,
      error: null,
    })
  })

  afterEach(cleanup)

  it('finishes setup without launching and launches only on the next request', async () => {
    checkMock.mockResolvedValueOnce({ ...readyStatus(), readyToLaunch: false })
    const { result } = renderHook(() => useLaunchGame(server))

    await act(async () => result.current.handleLaunch())

    expect(setupMock).toHaveBeenCalledTimes(1)
    expect(launchMock).not.toHaveBeenCalled()
    expect(useLauncherStore.getState()).toMatchObject({
      status: 'idle',
      clients: [],
    })
    expect(useSettingsStore.getState().advancedStatus?.readyToLaunch).toBe(true)

    await act(async () => result.current.handleLaunch())
    expect(launchMock).toHaveBeenCalledTimes(1)
  })

  it('preserves the bootstrap cause and retries without replacing it with missing registry files', async () => {
    const incomplete = {
      ...readyStatus(),
      readyToLaunch: false,
      prefixOk: false,
      prefixWarning: 'Falta system.reg · Falta user.reg',
    }
    checkMock.mockResolvedValue(incomplete)
    const primary =
      'Inicialización UMU/Steam Runtime/Proton: ERROR: Digest mismatched'
    setupMock.mockRejectedValueOnce(new Error(primary))
    const { result } = renderHook(() => useLaunchGame(server))
    await act(async () => result.current.handlePrepareEnvironment())
    expect(useLauncherStore.getState().error).toBe(primary)
    expect(checkMock).toHaveBeenCalledTimes(1)
    expect(launchMock).not.toHaveBeenCalled()
    checkMock
      .mockResolvedValueOnce(incomplete)
      .mockResolvedValueOnce(readyStatus())
    await act(async () => result.current.handlePrepareEnvironment())
    expect(setupMock).toHaveBeenCalledTimes(2)
    expect(useLauncherStore.getState().error).toBeNull()
    expect(launchMock).not.toHaveBeenCalled()
  })

  it('shows the actionable host blocker before secondary prefix diagnostics', async () => {
    checkMock.mockResolvedValue({
      ...readyStatus(),
      readyToLaunch: false,
      canSetup: false,
      runnerWarning: 'host-python: UMU necesita Python 3.10+',
      prefixWarning: 'Falta system.reg',
    })
    const { result } = renderHook(() => useLaunchGame(server))
    await act(async () => result.current.handlePrepareEnvironment())
    expect(useLauncherStore.getState().error).toBe(
      'host-python: UMU necesita Python 3.10+',
    )
    expect(setupMock).not.toHaveBeenCalled()
  })

  it('prepares from the button without opening launch fields or starting the game', async () => {
    const withFields: ServerConfig = {
      ...server,
      launch: { strategy: 'direct', gameArgs: ['${username}'] },
    }
    useServersStore.setState({ servers: [withFields] })
    checkMock.mockResolvedValueOnce({ ...readyStatus(), readyToLaunch: false })
    checkMock.mockResolvedValueOnce({ ...readyStatus(), readyToLaunch: false })
    render(<Launcher />)
    await screen.findByRole('button', { name: 'Preparar entorno' })

    fireEvent.click(screen.getByRole('button', { name: 'Preparar entorno' }))
    await screen.findByRole('button', { name: 'Jugar' })

    expect(setupMock).toHaveBeenCalledTimes(1)
    expect(launchMock).not.toHaveBeenCalled()
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it.each(['servers', 'runners'])(
    'recognizes an existing environment when %s finishes loading last',
    async (last) => {
      const pendingRunners = deferred<RunnerInfo[]>()
      const pendingServers = deferred<ServerConfig[]>()
      runnersMock.mockReturnValueOnce(pendingRunners.promise)
      serversMock.mockReturnValueOnce(pendingServers.promise)
      useSettingsStore.setState({ loading: true, selectedRunner: '' })
      useServersStore.setState({ loading: true, servers: [], selectedId: null })
      render(<Launcher />)
      expect(
        screen.getByRole('button', { name: 'Comprobando entorno...' }),
      ).toBeDisabled()
      expect(
        screen.queryByRole('button', { name: 'Preparar entorno' }),
      ).not.toBeInTheDocument()

      let init!: Promise<unknown>
      act(() => {
        init = Promise.all([
          useSettingsStore.getState().init(),
          useServersStore.getState().loadServers(),
        ])
      })
      const runners = [
        { id: MANAGED_RUNTIME_ID, name: 'Proton', path: '/opt/proton/proton' },
      ]
      await act(async () => {
        if (last === 'servers') pendingRunners.resolve(runners)
        else pendingServers.resolve([server])
      })
      expect(checkMock).not.toHaveBeenCalled()
      await act(async () => {
        pendingRunners.resolve(runners)
        pendingServers.resolve([server])
        await init
      })
      await screen.findByRole('button', { name: 'Jugar' })

      expect(checkMock).toHaveBeenCalledTimes(1)
      expect(checkMock).toHaveBeenCalledWith(server, '/opt/proton/proton')
      expect(setupMock).not.toHaveBeenCalled()
    },
  )

  it('shows a failed check as retryable instead of offering setup', async () => {
    checkMock.mockRejectedValueOnce(
      new Error('No se pudo comprobar el entorno'),
    )
    render(<Launcher />)
    const retry = await screen.findByRole('button', {
      name: 'Comprobar entorno',
    })
    expect(
      screen.queryByRole('button', { name: 'Preparar entorno' }),
    ).not.toBeInTheDocument()
    fireEvent.click(retry)
    await screen.findByRole('button', { name: 'Jugar' })
    expect(setupMock).not.toHaveBeenCalled()
    expect(launchMock).not.toHaveBeenCalled()
  })

  it('does not open an old credentials modal after switching servers during preflight', async () => {
    const withFields: ServerConfig = {
      ...server,
      launch: { strategy: 'direct', gameArgs: ['${username}'] },
    }
    const other: ServerConfig = { ...server, id: 'honey', name: 'HoneyRO' }
    useServersStore.setState({ servers: [withFields, other] })
    render(<Launcher />)
    await screen.findByRole('button', { name: 'Jugar' })
    const pending = deferred<DependencyStatus>()
    checkMock.mockReturnValueOnce(pending.promise)
    fireEvent.click(screen.getByRole('button', { name: 'Jugar' }))
    await screen.findByRole('button', { name: 'Comprobando...' })
    act(() => useServersStore.getState().selectServer(other.id))
    expect(
      screen.getByRole('button', { name: 'Comprobando...' }),
    ).toBeInTheDocument()
    await act(async () => pending.resolve(readyStatus()))
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(launchMock).not.toHaveBeenCalled()
    expect(setupMock).not.toHaveBeenCalled()
  })

  it('clears a cancelled launch when another server is selected', async () => {
    const other: ServerConfig = { ...server, id: 'honey', name: 'HoneyRO' }
    useServersStore.setState({ servers: [server, other] })
    render(<Launcher />)
    await screen.findByRole('button', { name: 'Jugar' })

    act(() => {
      useLauncherStore.setState({
        status: 'error',
        error: 'El lanzamiento fue cancelado por el usuario',
      })
    })
    expect(
      screen.getByRole('button', { name: 'Reintentar' }),
    ).toBeInTheDocument()
    expect(
      screen.getByText('El lanzamiento fue cancelado por el usuario'),
    ).toBeInTheDocument()

    act(() => useServersStore.getState().selectServer(other.id))
    await screen.findByRole('button', { name: 'Jugar' })
    expect(
      screen.queryByText('El lanzamiento fue cancelado por el usuario'),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: 'Reintentar' }),
    ).not.toBeInTheDocument()
  })

  it('keeps a cancelled launch after remounting on the same server', async () => {
    const { unmount } = render(<Launcher />)
    await screen.findByRole('button', { name: 'Jugar' })
    act(() => {
      useLauncherStore.setState({
        status: 'error',
        error: 'El lanzamiento fue cancelado por el usuario',
      })
    })
    unmount()
    render(<Launcher />)
    expect(
      screen.getByRole('button', { name: 'Reintentar' }),
    ).toBeInTheDocument()
    expect(
      screen.getByText('El lanzamiento fue cancelado por el usuario'),
    ).toBeInTheDocument()
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
    expect(useLauncherStore.getState().clients).toHaveLength(1)
    expect(useLauncherStore.getState().status).toBe('idle')
  })

  it('keeps operation ownership across an unmount and cancels its launch continuation', async () => {
    const pending = deferred<DependencyStatus>()
    checkMock.mockReturnValueOnce(pending.promise)
    const first = renderHook(() => useLaunchGame(server))
    let launch!: Promise<void>
    act(() => {
      launch = first.result.current.handleLaunch()
    })
    first.unmount()
    const second = renderHook(() => useLaunchGame(server))
    await act(async () => second.result.current.handleLaunch())
    expect(checkMock).toHaveBeenCalledTimes(1)
    pending.resolve(readyStatus())
    await act(async () => launch)
    expect(launchMock).not.toHaveBeenCalled()
    await act(async () => second.result.current.handleLaunch())
    expect(launchMock).toHaveBeenCalledTimes(1)
  })

  it('does not resurrect a client whose exit preceded the launch response', async () => {
    const pending = deferred<GameClientSnapshot>()
    launchMock.mockReturnValueOnce(pending.promise)
    const { result } = renderHook(() => useLaunchGame(server))
    let launch!: Promise<void>
    act(() => {
      launch = result.current.handleLaunch()
    })
    await vi.waitFor(() => expect(launchMock).toHaveBeenCalledTimes(1))
    const clientId = launchMock.mock.calls[0][0] as string
    act(() => useLauncherStore.getState().removeClient(clientId))
    pending.resolve({
      clientId,
      serverId: server.id,
      serverName: server.name,
      status: 'running',
      pid: 42,
    })
    await act(async () => launch)
    expect(useLauncherStore.getState().clients).toEqual([])
  })

  it('permits a new launch after the previous client is running', async () => {
    const { result } = renderHook(() => useLaunchGame(server))

    await act(async () => result.current.handleLaunch())
    await act(async () => result.current.handleLaunch())

    expect(launchMock).toHaveBeenCalledTimes(2)
    expect(useLauncherStore.getState().clients).toHaveLength(2)
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
