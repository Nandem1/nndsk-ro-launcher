// @vitest-environment jsdom

import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { MANAGED_RUNTIME_ID } from '../../shared/constants'
import type { ServerConfig } from '../../shared/types'
import { deferred } from '../../test/deferred'
import { useLauncherStore } from '../launcher/launcher.store'
import { useServersStore } from '../servers/servers.store'
import { RunnerSelector } from './RunnerSelector'
import { useSettingsStore } from './settings.store'

const { openMock } = vi.hoisted(() => ({ openMock: vi.fn() }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }))

const importAction = useSettingsStore.getState().importRuntimeArchive
const setRunnerAction = useSettingsStore.getState().setRunner
const current = {
  id: MANAGED_RUNTIME_ID,
  name: 'nndsk-ro-proton 0.1.0-dev.2',
  path: '/runtime/nndsk-ro-proton-0.1.0-dev.2/proton',
}
const server: ServerConfig = {
  id: 'sakura',
  name: 'SakuraRO',
  executablePath: '/games/sakura/ragexe.exe',
  runner: '/runners/wine716/bin/wine',
}

describe('RunnerSelector runtime import', () => {
  beforeEach(() => {
    openMock.mockReset()
    useSettingsStore.setState({
      runners: [
        current,
        {
          id: 'external:wine716',
          name: 'Wine 7.16 old-WoW64',
          path: server.runner!,
        },
      ],
      selectedRunner: current.path,
      savingRunner: false,
      importingRuntime: false,
      error: null,
      importRuntimeArchive: importAction,
      setRunner: setRunnerAction,
    })
    useServersStore.setState({ servers: [], selectedId: null })
    useLauncherStore.setState({ status: 'idle', clients: [] })
  })

  afterEach(() => {
    cleanup()
    vi.restoreAllMocks()
    useSettingsStore.setState({
      importRuntimeArchive: importAction,
      setRunner: setRunnerAction,
    })
  })

  it('opens the archive picker only on an explicit import action', async () => {
    const importing = vi.fn().mockResolvedValue(true)
    useSettingsStore.setState({ importRuntimeArchive: importing })
    openMock.mockResolvedValue('/tmp/nndsk-ro-proton.tar.zst')
    render(<RunnerSelector />)

    expect(openMock).not.toHaveBeenCalled()
    expect(importing).not.toHaveBeenCalled()
    fireEvent.click(
      screen.getByRole('button', { name: 'Importar y usar nndsk-ro-proton' }),
    )

    await waitFor(() =>
      expect(importing).toHaveBeenCalledWith('/tmp/nndsk-ro-proton.tar.zst'),
    )
    expect(openMock).toHaveBeenCalledWith({
      title: 'Importar nndsk-ro-proton verificado',
      multiple: false,
      directory: false,
      filters: [{ name: 'Runtime tar.zst', extensions: ['zst'] }],
    })
  })

  it('does not import or change selection after picker cancellation', async () => {
    const importing = vi.fn()
    const selecting = vi.fn()
    useSettingsStore.setState({
      importRuntimeArchive: importing,
      setRunner: selecting,
    })
    openMock.mockResolvedValue(null)
    render(<RunnerSelector />)
    fireEvent.click(
      screen.getByRole('button', { name: 'Importar y usar nndsk-ro-proton' }),
    )
    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: 'Importar y usar nndsk-ro-proton' }),
      ).toBeEnabled(),
    )
    expect(importing).not.toHaveBeenCalled()
    expect(selecting).not.toHaveBeenCalled()
  })

  it('preserves and displays the effective per-server Wine 7.16 override', () => {
    useServersStore.setState({ servers: [server], selectedId: server.id })
    render(<RunnerSelector />)

    expect(
      screen.getByText('Runner efectivo de SakuraRO: Wine 7.16 old-WoW64'),
    ).toBeInTheDocument()
    expect(
      screen.getByText(
        'Propio del servidor; el predeterminado global no lo reemplaza.',
      ),
    ).toBeInTheDocument()
    expect(useServersStore.getState().servers[0].runner).toBe(server.runner)
  })

  it('offers verified local import even if no runner is detected', () => {
    useSettingsStore.setState({ runners: [], selectedRunner: '' })
    render(<RunnerSelector />)
    expect(
      screen.getByRole('button', { name: 'Importar y usar nndsk-ro-proton' }),
    ).toBeEnabled()
    expect(
      screen.getByText(/se descarga y verifica por SHA-256 al preparar el/),
    ).toBeInTheDocument()
  })

  it('blocks runtime changes while the launcher is preparing a client', () => {
    useLauncherStore.setState({ status: 'setting-up' })
    render(<RunnerSelector />)
    expect(
      screen.getByRole('button', { name: 'Importar y usar nndsk-ro-proton' }),
    ).toBeDisabled()
    expect(
      screen.getByRole('button', { name: 'nndsk-ro-proton 0.1.0-dev.2' }),
    ).toBeDisabled()
  })

  it('reports a native dialog error without starting an import', async () => {
    const importing = vi.fn()
    useSettingsStore.setState({ importRuntimeArchive: importing })
    openMock.mockRejectedValue(new Error('dialog unavailable'))
    render(<RunnerSelector />)
    fireEvent.click(
      screen.getByRole('button', { name: 'Importar y usar nndsk-ro-proton' }),
    )
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'dialog unavailable',
    )
    expect(importing).not.toHaveBeenCalled()
  })

  it('rechecks launcher ownership when the native picker returns', async () => {
    const pending = deferred<string>()
    const importing = vi.fn()
    useSettingsStore.setState({ importRuntimeArchive: importing })
    openMock.mockReturnValueOnce(pending.promise)
    render(<RunnerSelector />)
    fireEvent.click(
      screen.getByRole('button', { name: 'Importar y usar nndsk-ro-proton' }),
    )
    useLauncherStore.setState({ status: 'launching' })
    pending.resolve('/tmp/runtime.tar.zst')
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Espera a que termine la sesión antes de importar el runtime',
    )
    expect(importing).not.toHaveBeenCalled()
  })
})
