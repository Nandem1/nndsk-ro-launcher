import { afterEach, describe, expect, it, vi } from 'vitest'

import { api } from '../../shared/api'
import { LEGACY_DEFAULT_WINE, MANAGED_RUNTIME_ID } from '../../shared/constants'
import type { RunnerInfo } from '../../shared/types'
import { deferred } from '../../test/deferred'
import { useSettingsStore } from './settings.store'

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
      savingRunner: false,
      error: null,
      notice: null,
    })
  })

  it('migrates an existing Wine selection to the managed runtime', async () => {
    vi.spyOn(api, 'listRunners').mockResolvedValue([proton])
    const saveSettings = vi.spyOn(api, 'saveSettings').mockResolvedValue()
    const loadDepsStatus = vi.fn().mockResolvedValue(undefined)
    useSettingsStore.setState({
      selectedRunner: LEGACY_DEFAULT_WINE,
      loadDepsStatus,
    })

    await useSettingsStore.getState().loadRunners()

    expect(useSettingsStore.getState().selectedRunner).toBe(proton.path)
    expect(useSettingsStore.getState().notice?.kind).toBe('migrated')
    expect(saveSettings).toHaveBeenCalledWith({ defaultRunner: proton.path })
    expect(loadDepsStatus).toHaveBeenCalledWith(proton.path)
  })

  it('selects the preferred runner for a fresh configuration', async () => {
    vi.spyOn(api, 'listRunners').mockResolvedValue([proton])
    vi.spyOn(api, 'saveSettings').mockResolvedValue()
    const loadDepsStatus = vi.fn().mockResolvedValue(undefined)
    useSettingsStore.setState({ selectedRunner: '', loadDepsStatus })

    await useSettingsStore.getState().loadRunners()

    expect(useSettingsStore.getState().selectedRunner).toBe(proton.path)
    expect(useSettingsStore.getState().notice?.kind).toBe('migrated')
    expect(loadDepsStatus).toHaveBeenCalledWith(proton.path)
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
})
