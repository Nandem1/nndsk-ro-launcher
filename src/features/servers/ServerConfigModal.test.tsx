// @vitest-environment jsdom

import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ServerConfig } from '../../shared/types'
import { useSettingsStore } from '../settings/settings.store'
import { AddServerModal } from './AddServerModal'
import { ServerConfigModal } from './ServerConfigModal'
import { ServerList } from './ServerList'
import { useServersStore } from './servers.store'

const { openMock } = vi.hoisted(() => ({ openMock: vi.fn() }))

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }))

const server: ServerConfig = {
  id: 'sakura',
  name: 'SakuraRO',
  executablePath: '/games/sakura/ragexe.exe',
}

describe('server configuration modal', () => {
  beforeEach(() => {
    openMock.mockReset()
    useSettingsStore.setState({
      runners: [
        { id: 'wine', name: 'Wine', path: '/usr/bin/wine' },
        { id: 'proton', name: 'Proton', path: '/opt/proton/proton' },
      ],
    })
    useServersStore.setState({
      servers: [server],
      selectedId: server.id,
      loading: false,
      error: null,
    })
  })

  afterEach(() => {
    cleanup()
    vi.restoreAllMocks()
  })

  it('adds a new server with an isolated prefix by default', async () => {
    const addServer = vi.fn().mockResolvedValue(undefined)
    const onClose = vi.fn()
    useServersStore.setState({ addServer })
    openMock.mockResolvedValue('/games/sakura/ragexe.exe')

    render(<AddServerModal onClose={onClose} />)

    fireEvent.click(
      screen.getByRole('button', { name: /Seleccionar \.exe.*Examinar/i }),
    )
    await screen.findByText('ragexe.exe')
    fireEvent.click(screen.getByRole('button', { name: 'Agregar' }))

    await waitFor(() => expect(addServer).toHaveBeenCalledTimes(1))
    expect(addServer).toHaveBeenCalledWith(
      expect.objectContaining({
        name: 'ragexe',
        executablePath: '/games/sakura/ragexe.exe',
        prefixMode: 'isolated',
        winePrefix: null,
        runner: null,
        launch: {
          strategy: 'direct',
          gameArgs: [],
          patcherArgs: [],
          requireWebview2: false,
        },
      }),
    )
    expect(onClose).toHaveBeenCalled()
  })

  it('requires a patcher when patcher strategy is selected', async () => {
    const onSave = vi.fn().mockResolvedValue(undefined)

    render(
      <ServerConfigModal
        mode="edit"
        server={server}
        onSave={onSave}
        onClose={vi.fn()}
      />,
    )

    fireEvent.click(
      screen.getByRole('button', {
        name: 'Ejecutar el juego directamente',
      }),
    )
    fireEvent.click(
      screen.getByRole('option', { name: 'Iniciar mediante el patcher' }),
    )
    fireEvent.click(screen.getByRole('button', { name: 'Guardar cambios' }))

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Selecciona un patcher para usar la estrategia Patcher',
    )
    expect(onSave).not.toHaveBeenCalled()
  })

  it('exposes an accessible edit action for each server', () => {
    render(<ServerList />)

    fireEvent.click(screen.getByRole('button', { name: 'Editar SakuraRO' }))

    expect(
      screen.getByRole('dialog', { name: 'Editar servidor' }),
    ).toBeInTheDocument()
  })
})
