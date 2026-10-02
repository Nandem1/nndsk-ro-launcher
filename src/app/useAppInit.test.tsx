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
    vi.mocked(invoke).mockImplementation(async (command) =>
      command === 'take_storage_notices' ? [] : undefined,
    )
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
    vi.mocked(invoke).mockImplementation(async (command) =>
      command === 'take_storage_notices' ? [] : undefined,
    )
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
    expect(
      vi
        .mocked(invoke)
        .mock.calls.filter(([command]) => command === 'show_main_window'),
    ).toHaveLength(2)
  })

  it('keeps ready phase when storage recovery emits a notice', async () => {
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === 'take_storage_notices') {
        return [
          {
            source: 'servers',
            kind: 'recovered',
            message: 'Servidores recuperados',
          },
        ]
      }
      return undefined
    })
    vi.spyOn(useServersStore.getState(), 'loadServers').mockResolvedValue(true)
    vi.spyOn(useSettingsStore.getState(), 'init').mockResolvedValue(true)
    useSettingsStore.setState({ notice: null })

    const { result } = renderHook(() => useAppInit())

    await waitFor(() => expect(result.current.phase).toBe('ready'))
    expect(result.current.notices).toHaveLength(1)
    expect(result.current.errors).toEqual([])

    act(() => result.current.dismissNotices())
    expect(result.current.notices).toEqual([])
  })

  it('does not let an older retry overwrite a newer successful initialization', async () => {
    vi.mocked(invoke).mockImplementation(async (command) =>
      command === 'take_storage_notices' ? [] : undefined,
    )
    const load = vi
      .spyOn(useServersStore.getState(), 'loadServers')
      .mockResolvedValue(true)
    vi.spyOn(useSettingsStore.getState(), 'init').mockResolvedValue(true)
    const hook = renderHook(useAppInit)
    await waitFor(() => expect(hook.result.current.phase).toBe('ready'))
    const older = deferred<boolean>()
    load.mockReturnValueOnce(older.promise).mockResolvedValueOnce(true)
    let oldRetry!: Promise<void>
    act(() => {
      oldRetry = hook.result.current.retry()
    })
    await act(async () => {
      await hook.result.current.retry()
    })
    await act(async () => {
      older.resolve(false)
      await oldRetry
    })
    expect(hook.result.current.phase).toBe('ready')
    expect(hook.result.current.errors).toEqual([])
    hook.unmount()
  })

  it('does not show a window or consume notices after its initialization owner unmounts', async () => {
    const pending = deferred<boolean>()
    vi.spyOn(useServersStore.getState(), 'loadServers').mockReturnValueOnce(
      pending.promise,
    )
    vi.spyOn(useSettingsStore.getState(), 'init').mockResolvedValue(true)
    const hook = renderHook(useAppInit)
    hook.unmount()
    await act(async () => {
      pending.resolve(true)
    })
    expect(invoke).not.toHaveBeenCalled()
  })
})
