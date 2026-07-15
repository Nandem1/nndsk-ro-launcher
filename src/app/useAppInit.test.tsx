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
    vi.mocked(invoke).mockReset()
  })

  it('waits for servers and settings before showing the main window', async () => {
    const servers = deferred<boolean>()
    const settings = deferred<boolean>()
    vi.mocked(invoke).mockResolvedValue(undefined)
    vi.spyOn(useServersStore.getState(), 'loadServers').mockReturnValue(
      servers.promise,
    )
    vi.spyOn(useSettingsStore.getState(), 'init').mockReturnValue(
      settings.promise,
    )

    const { result } = renderHook(() => useAppInit())

    expect(result.current.phase).toBe('loading')
    expect(invoke).not.toHaveBeenCalled()

    await act(async () => {
      servers.resolve(true)
      settings.resolve(true)
      await Promise.all([servers.promise, settings.promise])
    })

    await waitFor(() => expect(result.current.phase).toBe('ready'))
    expect(invoke).toHaveBeenCalledWith('show_main_window')
  })

  it('opens in degraded mode and can retry failed initialization', async () => {
    vi.mocked(invoke).mockResolvedValue(undefined)
    vi.spyOn(useServersStore.getState(), 'loadServers')
      .mockImplementationOnce(async () => {
        useServersStore.setState({ error: 'servers unavailable' })
        return false
      })
      .mockResolvedValueOnce(true)
    vi.spyOn(useSettingsStore.getState(), 'init').mockResolvedValue(true)

    const { result } = renderHook(() => useAppInit())

    await waitFor(() => expect(result.current.phase).toBe('degraded'))
    expect(result.current.errors).toContain('servers unavailable')

    await act(async () => {
      await result.current.retry()
    })

    expect(result.current.phase).toBe('ready')
    expect(result.current.errors).toEqual([])
    expect(invoke).toHaveBeenCalledTimes(2)
  })
})
