// @vitest-environment jsdom

import { invoke } from '@tauri-apps/api/core'
import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { useServersStore } from '../features/servers/servers.store'
import { useSettingsStore } from '../features/settings/settings.store'
import { deferred } from '../test/deferred'
import { useAppInit } from './useAppInit'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

describe('useAppInit', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('waits for servers and settings before showing the main window', async () => {
    const servers = deferred<void>()
    const settings = deferred<void>()
    vi.spyOn(useServersStore.getState(), 'loadServers').mockReturnValue(
      servers.promise,
    )
    vi.spyOn(useSettingsStore.getState(), 'init').mockReturnValue(
      settings.promise,
    )

    const { result } = renderHook(() => useAppInit())

    expect(result.current.ready).toBe(false)
    expect(invoke).not.toHaveBeenCalled()

    await act(async () => {
      servers.resolve()
      settings.resolve()
      await Promise.all([servers.promise, settings.promise])
    })

    await waitFor(() => expect(result.current.ready).toBe(true))
    expect(invoke).toHaveBeenCalledWith('show_main_window')
  })
})
