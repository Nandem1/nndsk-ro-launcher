// @vitest-environment jsdom

import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { ServerConfig, ServerToolsStatus } from '../../shared/types'
import { useSettingsStore } from '../settings/settings.store'
import { useServerTools } from './useServerTools'

const { launchToolMock, scanMock } = vi.hoisted(() => ({
  launchToolMock: vi.fn(),
  scanMock: vi.fn(),
}))

vi.mock('../../shared/api', () => ({
  api: {
    launchServerTool: launchToolMock,
    scanServerTools: scanMock,
  },
}))

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => {
    resolve = done
  })
  return { promise, resolve }
}

function status(gameDir: string): ServerToolsStatus {
  const missing = { found: false, path: null, label: null }
  return {
    gameDir,
    openSetup: missing,
    patcher: missing,
    dgvoodoo: {
      cpl: missing,
      d3dimmDll: missing,
      ddrawDll: missing,
      conf: missing,
      configured: false,
      needsInstall: true,
      canAutoInstall: true,
      canUninstall: false,
      issues: [],
    },
    diagnostics: {
      architecture: null,
      graphicsApis: [],
      managedPatcher: false,
      webview2Required: false,
      peAnalysisConclusive: false,
      gepardPresent: false,
      gameguardPresent: false,
      warnings: [],
    },
  }
}

const serverA: ServerConfig = {
  id: 'a',
  name: 'A',
  executablePath: '/games/a/a.exe',
}
const serverB: ServerConfig = {
  id: 'b',
  name: 'B',
  executablePath: '/games/b/b.exe',
}

describe('useServerTools', () => {
  beforeEach(() => {
    scanMock.mockReset()
    launchToolMock.mockReset().mockResolvedValue(undefined)
    useSettingsStore.setState({ selectedRunner: '/usr/bin/wine' })
  })

  it('ignores a stale scan after switching servers', async () => {
    const first = deferred<ServerToolsStatus>()
    const second = deferred<ServerToolsStatus>()
    scanMock
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)

    const { result, rerender } = renderHook(
      ({ server }) => useServerTools(server),
      { initialProps: { server: serverA } },
    )
    rerender({ server: serverB })

    await act(async () => second.resolve(status('/games/b')))
    await waitFor(() => expect(result.current.status?.gameDir).toBe('/games/b'))

    await act(async () => first.resolve(status('/games/a')))
    expect(result.current.status?.gameDir).toBe('/games/b')
  })

  it('passes an explicit runner snapshot when opening a tool', async () => {
    const scanned = status('/games/a')
    scanned.openSetup = {
      found: true,
      path: '/games/a/OpenSetup.exe',
      label: 'OpenSetup.exe',
    }
    scanMock.mockResolvedValue(scanned)
    const { result } = renderHook(() => useServerTools(serverA))
    await waitFor(() => expect(result.current.status).not.toBeNull())

    await act(async () => result.current.handleOpen('opensetup'))

    expect(launchToolMock).toHaveBeenCalledWith(
      serverA,
      'opensetup',
      '/usr/bin/wine',
    )
  })
})
