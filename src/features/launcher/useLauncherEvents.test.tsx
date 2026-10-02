// @vitest-environment jsdom
import { act, cleanup, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ExitEventPayload, GameClientSnapshot } from '../../shared/types'
import { deferred } from '../../test/deferred'
import { api } from '../../shared/api'
import { LAUNCHER_EVENTS } from '../../shared/constants'
import { useLauncherStore } from './launcher.store'
import { useLauncherEvents } from './useLauncherEvents'
import { refreshGameClients } from './refreshGameClients'

const { handlers, subscriptions } = vi.hoisted(() => ({
  handlers: new Map<string, (payload: unknown) => void>(),
  subscriptions: { ready: true },
}))
vi.mock('../../shared/hooks/useTauriEvent', () => ({
  useTauriEvent: (event: string, handler: (payload: unknown) => void) => {
    handlers.set(event, handler)
    return subscriptions.ready
  },
}))

const client: GameClientSnapshot = {
  clientId: 'old',
  serverId: 'srv',
  serverName: 'RO',
  status: 'running',
  pid: 12,
}
const exit: ExitEventPayload = {
  clientId: 'old',
  serverId: 'srv',
  serverName: 'RO',
  code: 1,
  requested: false,
}

describe('launcher event ordering', () => {
  beforeEach(() => {
    subscriptions.ready = true
    useLauncherStore.setState({
      status: 'idle',
      clients: [client],
      clientsRevision: 0,
      closedClientIds: [],
      error: null,
    })
    vi.spyOn(api, 'listGameClients').mockResolvedValue([client])
  })
  afterEach(() => {
    cleanup()
    vi.restoreAllMocks()
    handlers.clear()
  })

  it.each(['checking', 'setting-up', 'launching'] as const)(
    'keeps %s owned when another client exits',
    async (status) => {
      renderHook(useLauncherEvents)
      await act(async () => {})
      act(() => {
        useLauncherStore.setState({ status })
        handlers.get(LAUNCHER_EVENTS.GAME_EXIT)!(exit)
      })
      expect(useLauncherStore.getState()).toMatchObject({
        status,
        error: null,
        clients: [],
      })
    },
  )

  it('subscribes to lifecycle events before requesting the initial client census', async () => {
    subscriptions.ready = false
    const hook = renderHook(useLauncherEvents)
    expect(api.listGameClients).not.toHaveBeenCalled()
    subscriptions.ready = true
    hook.rerender()
    await act(async () => {})
    expect(api.listGameClients).toHaveBeenCalledTimes(1)
  })

  it('discards a stale census after a client event and a late resurrection event', async () => {
    const pending = deferred<GameClientSnapshot[]>()
    vi.mocked(api.listGameClients).mockReturnValueOnce(pending.promise)
    renderHook(useLauncherEvents)
    act(() => {
      handlers.get(LAUNCHER_EVENTS.GAME_EXIT)!(exit)
      handlers.get(LAUNCHER_EVENTS.GAME_CLIENT)!(client)
    })
    await act(async () => pending.resolve([client]))
    expect(useLauncherStore.getState().clients).toEqual([])
  })

  it('discards an older census even when no events changed the revision', async () => {
    const older = deferred<GameClientSnapshot[]>()
    const newer = deferred<GameClientSnapshot[]>()
    vi.mocked(api.listGameClients)
      .mockReturnValueOnce(older.promise)
      .mockReturnValueOnce(newer.promise)
    const first = refreshGameClients()
    const second = refreshGameClients()
    newer.resolve([])
    await second
    older.resolve([client])
    await first
    expect(useLauncherStore.getState().clients).toEqual([])
  })

  it('rejects delayed progress from an earlier setup while a new setup is active', async () => {
    renderHook(useLauncherEvents)
    await act(async () => {})
    const progress = { step: 'Current', percent: 40, operationId: 'new' }
    act(() => {
      useLauncherStore.setState({
        status: 'setting-up',
        operationId: 'new',
        setupProgress: progress,
      })
      handlers.get(LAUNCHER_EVENTS.PROGRESS)!({
        step: 'Old complete',
        percent: 100,
        operationId: 'old',
      })
      handlers.get(LAUNCHER_EVENTS.PROGRESS)!({
        step: 'Uncorrelated',
        percent: 100,
      })
    })
    expect(useLauncherStore.getState().setupProgress).toEqual(progress)
    act(() =>
      handlers.get(LAUNCHER_EVENTS.PROGRESS)!({ ...progress, percent: 60 }),
    )
    expect(useLauncherStore.getState().setupProgress?.percent).toBe(60)
  })
})
