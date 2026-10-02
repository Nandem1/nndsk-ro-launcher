import { afterEach, describe, expect, it, vi } from 'vitest'

import { api } from '../../shared/api'
import { LEGACY_DEFAULT_WINE, MANAGED_RUNTIME_ID } from '../../shared/constants'
import type { RunnerInfo } from '../../shared/types'
import { deferred } from '../../test/deferred'
import { useSettingsStore } from './settings.store'
import type { DependencyStatus } from '../../shared/types'
import { runtimeStatusKey } from '../../shared/resolveRunner'

const loadDepsStatusAction = useSettingsStore.getState().loadDepsStatus

const proton: RunnerInfo = {
  id: MANAGED_RUNTIME_ID,
  name: 'Proton recomendado',
  path: '/opt/proton/proton',
}

describe('settings store runner selection', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    useSettingsStore.setState({
      runners: [],
      selectedRunner: '',
      richPresenceEnabled: false,
      savingRunner: false,
      savingPresence: false,
      error: null,
      notice: null,
      loadDepsStatus: loadDepsStatusAction,
    })
  })

  it('preserves an unavailable persisted Wine selection without saving', async () => {
    vi.spyOn(api, 'listRunners').mockResolvedValue([proton])
    const saveSettings = vi.spyOn(api, 'saveSettings').mockResolvedValue()
    const loadDepsStatus = vi.fn().mockResolvedValue(undefined)
    useSettingsStore.setState({
      selectedRunner: LEGACY_DEFAULT_WINE,
      loadDepsStatus,
    })

    await useSettingsStore.getState().loadRunners()

    expect(useSettingsStore.getState().selectedRunner).toBe(LEGACY_DEFAULT_WINE)
    expect(useSettingsStore.getState().notice).toBeNull()
    expect(saveSettings).not.toHaveBeenCalled()
    expect(loadDepsStatus).not.toHaveBeenCalled()
  })

  it('selects the preferred runner for a fresh configuration', async () => {
    vi.spyOn(api, 'listRunners').mockResolvedValue([proton])
    vi.spyOn(api, 'saveSettings').mockResolvedValue()
    const loadDepsStatus = vi.fn().mockResolvedValue(undefined)
    useSettingsStore.setState({ selectedRunner: '', loadDepsStatus })

    await useSettingsStore.getState().loadRunners()

    expect(useSettingsStore.getState().selectedRunner).toBe(proton.path)
    expect(useSettingsStore.getState().notice?.kind).toBe('migrated')
    expect(loadDepsStatus).not.toHaveBeenCalled()
  })

  it('serializes rapid runner writes and keeps the latest selection', async () => {
    const first = deferred<void>()
    const second = deferred<void>()
    const save = vi
      .spyOn(api, 'saveSettings')
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
    useSettingsStore.setState({
      selectedRunner: '/usr/bin/wine',
      savingRunner: false,
    })

    const selectA = useSettingsStore
      .getState()
      .setRunner('/opt/proton-a/proton')
    const selectB = useSettingsStore
      .getState()
      .setRunner('/opt/proton-b/proton')
    await vi.waitFor(() => expect(save).toHaveBeenCalledTimes(1))

    first.resolve()
    await vi.waitFor(() => expect(save).toHaveBeenCalledTimes(2))
    second.resolve()
    await Promise.all([selectA, selectB])

    expect(save.mock.calls.map(([settings]) => settings.defaultRunner)).toEqual(
      ['/opt/proton-a/proton', '/opt/proton-b/proton'],
    )
    expect(useSettingsStore.getState()).toMatchObject({
      selectedRunner: '/opt/proton-b/proton',
      savingRunner: false,
      error: null,
    })
  })

  it('persists the Rich Presence preference with the selected runner', async () => {
    const saveSettings = vi.spyOn(api, 'saveSettings').mockResolvedValue()
    useSettingsStore.setState({
      selectedRunner: '/opt/proton/proton',
      richPresenceEnabled: false,
    })

    await useSettingsStore.getState().setRichPresenceEnabled(true)

    expect(saveSettings).toHaveBeenCalledWith({
      defaultRunner: '/opt/proton/proton',
      richPresenceEnabled: true,
    })
    expect(useSettingsStore.getState()).toMatchObject({
      richPresenceEnabled: true,
      savingPresence: false,
      error: null,
    })
  })

  it('restores the persisted presence value after multiple failed optimistic writes', async () => {
    const pending = deferred<void>()
    vi.spyOn(api, 'saveSettings')
      .mockReturnValueOnce(pending.promise)
      .mockRejectedValueOnce(new Error('disk full'))
    useSettingsStore.setState({
      richPresenceEnabled: false,
      savingPresence: false,
    })
    const first = useSettingsStore.getState().setRichPresenceEnabled(true)
    const second = useSettingsStore.getState().setRichPresenceEnabled(false)
    pending.reject(new Error('disk full'))
    await Promise.all([first, second])
    expect(useSettingsStore.getState().richPresenceEnabled).toBe(false)
  })

  it('keeps runner persistence coherent when presence is queued between runner writes', async () => {
    const pending = deferred<void>()
    const save = vi
      .spyOn(api, 'saveSettings')
      .mockReturnValueOnce(pending.promise)
      .mockResolvedValue()
    useSettingsStore.setState({
      selectedRunner: '/usr/bin/wine',
      savingRunner: false,
    })
    const first = useSettingsStore.getState().setRunner('/opt/a/proton')
    const presence = useSettingsStore.getState().setRichPresenceEnabled(true)
    const last = useSettingsStore.getState().setRunner('/opt/b/proton')
    await vi.waitFor(() => expect(save).toHaveBeenCalledTimes(1))
    pending.resolve()
    await Promise.all([first, presence, last])
    expect(save.mock.calls.map(([settings]) => settings.defaultRunner)).toEqual(
      ['/opt/a/proton', '/opt/a/proton', '/opt/b/proton'],
    )
  })

  it('does not let a pending settings read overwrite a user selection', async () => {
    const pending = deferred<{ defaultRunner: string }>()
    vi.spyOn(api, 'loadSettings').mockReturnValueOnce(pending.promise)
    vi.spyOn(api, 'saveSettings').mockResolvedValue()
    const read = useSettingsStore.getState().loadSettings()
    await useSettingsStore.getState().setRunner('/opt/new/proton')
    pending.resolve({ defaultRunner: '/usr/bin/wine' })
    await read
    expect(useSettingsStore.getState().selectedRunner).toBe('/opt/new/proton')
  })

  it('rolls back to the complete last persisted snapshot across runner and presence writes', async () => {
    const pending = deferred<void>()
    const save = vi
      .spyOn(api, 'saveSettings')
      .mockReturnValueOnce(pending.promise)
      .mockResolvedValueOnce()
      .mockResolvedValueOnce()
      .mockRejectedValueOnce(new Error('disk full'))
    useSettingsStore.setState({
      selectedRunner: '/usr/bin/wine',
      richPresenceEnabled: false,
    })
    const runnerA = useSettingsStore.getState().setRunner('/opt/a/proton')
    await vi.waitFor(() => expect(save).toHaveBeenCalledTimes(1))
    const enable = useSettingsStore.getState().setRichPresenceEnabled(true)
    const runnerB = useSettingsStore.getState().setRunner('/opt/b/proton')
    const disable = useSettingsStore.getState().setRichPresenceEnabled(false)
    pending.resolve()
    await Promise.all([runnerA, enable, runnerB, disable])
    expect(save.mock.calls[2][0]).toEqual({
      defaultRunner: '/opt/b/proton',
      richPresenceEnabled: false,
    })
    expect(useSettingsStore.getState()).toMatchObject({
      selectedRunner: '/opt/b/proton',
      richPresenceEnabled: false,
    })
  })

  it('does not let an older runner listing replace a newer one', async () => {
    const older = deferred<RunnerInfo[]>()
    vi.spyOn(api, 'listRunners')
      .mockReturnValueOnce(older.promise)
      .mockResolvedValueOnce([proton])
    useSettingsStore.setState({ selectedRunner: proton.path })
    const first = useSettingsStore.getState().loadRunners()
    await useSettingsStore.getState().loadRunners()
    older.resolve([])
    await first
    expect(useSettingsStore.getState().runners).toEqual([proton])
  })

  it('discards an old initialization failure after a successful user settings write', async () => {
    const pending = deferred<{ defaultRunner: string }>()
    vi.spyOn(api, 'loadSettings').mockReturnValueOnce(pending.promise)
    vi.spyOn(api, 'listRunners').mockResolvedValue([proton])
    vi.spyOn(api, 'saveSettings').mockResolvedValue()
    const init = useSettingsStore.getState().init()
    await useSettingsStore.getState().setRunner(proton.path)
    pending.reject(new Error('old read failed'))
    await init
    expect(useSettingsStore.getState()).toMatchObject({
      selectedRunner: proton.path,
      error: null,
      loading: false,
    })
  })

  it('discards an older failed diagnosis after the final preparation diagnosis', async () => {
    const pending = deferred<DependencyStatus>()
    vi.spyOn(api, 'checkDependencies').mockReturnValueOnce(pending.promise)
    const read = loadDepsStatusAction(proton.path)
    const key = runtimeStatusKey(null, proton.path)
    useSettingsStore
      .getState()
      .applyDepsStatus({ readyToLaunch: true } as DependencyStatus, key)
    pending.reject(new Error('old failure'))
    await read
    expect(useSettingsStore.getState()).toMatchObject({
      advancedStatusKey: key,
      advancedStatusError: null,
      advancedStatus: { readyToLaunch: true },
    })
  })
})
