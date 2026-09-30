// @vitest-environment jsdom

import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { deferred } from '../../test/deferred'
import { useAppInit } from '../../app/useAppInit'
import { useServersStore } from '../servers/servers.store'
import { useSettingsStore } from '../settings/settings.store'
import { useUpdateBootstrap } from './useUpdateBootstrap'
import { useUpdaterStore } from './updater.store'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

describe('useUpdateBootstrap', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    useUpdaterStore.setState({
      snapshot: null,
      busy: false,
      error: null,
    })
  })

  it('does not delay useAppInit', async () => {
    const { invoke } = await import('@tauri-apps/api/core')
    const check = deferred<void>()
    vi.spyOn(useUpdaterStore.getState(), 'check').mockReturnValue(check.promise)
    vi.spyOn(useUpdaterStore.getState(), 'refresh').mockResolvedValue()
    vi.mocked(invoke).mockImplementation(async (command) =>
      command === 'take_storage_notices' ? [] : undefined,
    )
    vi.spyOn(useServersStore.getState(), 'loadServers').mockResolvedValue(true)
    vi.spyOn(useSettingsStore.getState(), 'init').mockResolvedValue(true)

    const init = renderHook(() => useAppInit())
    const bootstrap = renderHook(() => useUpdateBootstrap(false))

    await waitFor(() => expect(init.result.current.phase).toBe('ready'))
    expect(invoke).toHaveBeenCalledWith('show_main_window')
    expect(useUpdaterStore.getState().check).not.toHaveBeenCalled()

    await act(async () => {
      bootstrap.rerender()
      renderHook(() => useUpdateBootstrap(true))
    })

    await waitFor(() =>
      expect(useUpdaterStore.getState().check).toHaveBeenCalled(),
    )
    check.resolve()
  })
})
