// @vitest-environment jsdom
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { InstallationDraft } from '../../shared/types'
import { api } from '../../shared/api'
import { useSettingsStore } from '../settings/settings.store'
import { useServersStore } from '../servers/servers.store'
import { AddServerModal } from '../servers/AddServerModal'
import { InstallationWizard } from './InstallationWizard'
import { useInstallationsStore } from './installations.store'

const { openMock } = vi.hoisted(() => ({ openMock: vi.fn() }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }))

const draft: InstallationDraft = {
  id: 'install-1',
  serverId: 'server-1',
  name: 'TestRO',
  installerPath: '/downloads/Setup.exe',
  destinationPath: '/games/test',
  runner: '/managed/wine',
  requireWebview2: false,
  status: 'pending',
  binding: null,
  exitCode: null,
  message: null,
}

describe('installation wizard', () => {
  beforeEach(() => {
    openMock.mockReset()
    useSettingsStore.setState({
      selectedRunner: '/managed/wine',
      runners: [
        { id: 'proton', name: 'Proton', path: '/managed/proton' },
        { id: 'wine', name: 'Wine 7.16', path: '/managed/wine' },
      ],
    })
    useInstallationsStore.setState({ drafts: [], error: null })
  })
  afterEach(() => {
    cleanup()
    vi.restoreAllMocks()
  })

  it('opens the installer flow from add-server and reserves it before execution', async () => {
    const create = vi.spyOn(api, 'createInstallation').mockResolvedValue(draft)
    const start = vi
      .spyOn(api, 'startInstallation')
      .mockResolvedValue({ ...draft, status: 'preparing' })
    render(<AddServerModal onClose={vi.fn()} />)
    expect(
      screen.queryByText('Ya tengo el juego descomprimido'),
    ).not.toBeInTheDocument()
    const installerButton = screen.getByRole('button', {
      name: 'Tengo un instalador',
    })
    expect(
      screen.getByRole('heading', { name: 'Cliente' }).parentElement,
    ).toContainElement(installerButton)
    fireEvent.click(installerButton)
    openMock
      .mockResolvedValueOnce('/downloads/Setup.exe')
      .mockResolvedValueOnce('/games/test')
    fireEvent.click(
      screen.getByRole('button', { name: 'Seleccionar instalador .exe' }),
    )
    await screen.findByText('Setup.exe')
    fireEvent.change(screen.getByLabelText('Nombre del servidor'), {
      target: { value: 'TestRO' },
    })
    fireEvent.click(screen.getByText('Opciones avanzadas'))
    fireEvent.click(
      screen.getByRole('button', { name: 'Elegir carpeta externa (opcional)' }),
    )
    await screen.findByText('/games/test')
    fireEvent.click(
      screen.getByRole('button', { name: 'Preparar y abrir instalador' }),
    )
    await waitFor(() => expect(start).toHaveBeenCalledWith('install-1'))
    expect(create).toHaveBeenCalledWith(
      expect.objectContaining({
        name: 'TestRO',
        runner: '/managed/wine',
        installerPath: '/downloads/Setup.exe',
      }),
    )
    expect(create.mock.invocationCallOrder[0]).toBeLessThan(
      start.mock.invocationCallOrder[0],
    )
    expect(
      screen.queryByRole('button', { name: 'Agregar servidor instalado' }),
    ).not.toBeInTheDocument()
  })

  it('can close an active installation and resume with its fixed runner', () => {
    useInstallationsStore.setState({
      drafts: [{ ...draft, status: 'running' }],
    })
    useSettingsStore.setState({ selectedRunner: '/managed/proton' })
    const close = vi.fn()
    render(<InstallationWizard installationId={draft.id} onClose={close} />)
    expect(screen.getByText('Runner: Wine 7.16')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Cerrar instalación' }))
    expect(close).toHaveBeenCalledOnce()
  })

  it('opens the default installer flow without requesting an external directory', async () => {
    const create = vi.spyOn(api, 'createInstallation').mockResolvedValue(draft)
    vi.spyOn(api, 'startInstallation').mockResolvedValue({
      ...draft,
      status: 'preparing',
    })
    openMock.mockResolvedValue('/downloads/Setup.exe')
    render(<InstallationWizard onClose={vi.fn()} />)
    fireEvent.click(
      screen.getByRole('button', { name: 'Seleccionar instalador .exe' }),
    )
    await screen.findByText('Setup.exe')
    fireEvent.click(
      screen.getByRole('button', { name: 'Preparar y abrir instalador' }),
    )
    await waitFor(() => expect(create).toHaveBeenCalled())
    expect(create).toHaveBeenCalledWith(
      expect.objectContaining({ destinationPath: null }),
    )
    expect(openMock).toHaveBeenCalledOnce()
  })

  it('can reopen an installer that exited zero without installing a client', async () => {
    const waiting = { ...draft, status: 'awaitingClient' as const, exitCode: 0 }
    useInstallationsStore.setState({ drafts: [waiting] })
    const start = vi.spyOn(api, 'startInstallation').mockResolvedValue({
      ...waiting,
      status: 'preparing',
      exitCode: null,
    })
    render(<InstallationWizard installationId={draft.id} onClose={vi.fn()} />)
    expect(screen.getByText(/no confirma que el juego/i)).toBeInTheDocument()
    fireEvent.click(
      screen.getByRole('button', { name: 'Volver a abrir instalador' }),
    )
    await waitFor(() => expect(start).toHaveBeenCalledWith(draft.id))
    expect(
      screen.queryByRole('button', { name: 'Agregar servidor instalado' }),
    ).not.toBeInTheDocument()
  })

  it('requires explicit client selection before registering a successful installation', async () => {
    useInstallationsStore.setState({
      drafts: [{ ...draft, status: 'awaitingClient', exitCode: 0 }],
    })
    const register = vi.fn().mockResolvedValue(undefined)
    useServersStore.setState({ registerInstallation: register })
    vi.spyOn(api, 'listInstallations').mockResolvedValue([])
    openMock.mockResolvedValue('/games/test/ragexe.exe')
    render(<InstallationWizard installationId={draft.id} onClose={vi.fn()} />)
    expect(
      screen.getByRole('button', { name: 'Agregar servidor instalado' }),
    ).toBeDisabled()
    fireEvent.click(
      screen.getByRole('button', { name: 'Seleccionar ejecutable del juego' }),
    )
    await screen.findByText('/games/test/ragexe.exe')
    fireEvent.click(
      screen.getByRole('button', { name: 'Agregar servidor instalado' }),
    )
    await waitFor(() =>
      expect(register).toHaveBeenCalledWith(
        draft.id,
        '/games/test/ragexe.exe',
        null,
      ),
    )
  })

  it('starts the client picker inside C: rather than an optional external directory', async () => {
    useInstallationsStore.setState({
      drafts: [
        {
          ...draft,
          status: 'awaitingClient',
          exitCode: 0,
          binding: {
            installationId: draft.id,
            prefixPath: '/managed/prefix',
            prefixFingerprint: 'a'.repeat(64),
          },
        },
      ],
    })
    openMock.mockResolvedValue(null)
    render(<InstallationWizard installationId={draft.id} onClose={vi.fn()} />)
    fireEvent.click(
      screen.getByRole('button', { name: 'Seleccionar ejecutable del juego' }),
    )
    await waitFor(() =>
      expect(openMock).toHaveBeenCalledWith(
        expect.objectContaining({ defaultPath: '/managed/prefix/drive_c/' }),
      ),
    )
  })

  it('keeps empty search results recoverable and clears stale selections after reopening', async () => {
    const waiting = { ...draft, status: 'awaitingClient' as const, exitCode: 0 }
    useInstallationsStore.setState({ drafts: [waiting] })
    vi.spyOn(api, 'scanInstallationCandidates').mockResolvedValue([])
    vi.spyOn(api, 'startInstallation').mockResolvedValue({
      ...waiting,
      status: 'preparing',
      exitCode: null,
    })
    openMock.mockResolvedValue('/games/test/ragexe.exe')
    const register = vi.fn()
    useServersStore.setState({ registerInstallation: register })
    render(<InstallationWizard installationId={draft.id} onClose={vi.fn()} />)
    fireEvent.click(
      screen.getByRole('button', { name: 'Buscar ejecutables instalados' }),
    )
    await screen.findByText(/No se encontraron ejecutables candidatos/)
    fireEvent.click(
      screen.getByRole('button', { name: 'Seleccionar ejecutable del juego' }),
    )
    await screen.findByText('/games/test/ragexe.exe')
    fireEvent.click(
      screen.getByRole('button', { name: 'Volver a abrir instalador' }),
    )
    await screen.findByText(/Preparando el entorno/)
    useInstallationsStore.setState({ drafts: [waiting] })
    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: 'Agregar servidor instalado' }),
      ).toBeDisabled(),
    )
    expect(screen.queryByText('/games/test/ragexe.exe')).not.toBeInTheDocument()
    expect(
      screen.queryByText(/No se encontraron ejecutables candidatos/),
    ).not.toBeInTheDocument()
    expect(register).not.toHaveBeenCalled()
  })

  it('does not offer reopening a registered installation', () => {
    useInstallationsStore.setState({
      drafts: [{ ...draft, status: 'completed', exitCode: 0 }],
    })
    render(<InstallationWizard installationId={draft.id} onClose={vi.fn()} />)
    expect(
      screen.queryByRole('button', { name: 'Volver a abrir instalador' }),
    ).not.toBeInTheDocument()
  })
})
