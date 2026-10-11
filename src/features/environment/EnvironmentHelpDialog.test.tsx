// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { EnvironmentAssessment } from '../../shared/types'
import { EnvironmentHelpDialog } from './EnvironmentHelpDialog'
import { preparationFailure } from './environment.presentation'
import { missingTools } from './environment.fixtures'

function props(assessment: EnvironmentAssessment = missingTools) {
  return {
    serverName: 'SakuraRO',
    assessment,
    checking: false,
    busy: false,
    error: null,
    onClose: vi.fn(),
    onRecheck: vi.fn(),
    onPrepare: vi.fn(),
  }
}

afterEach(cleanup)

describe('environment assistance', () => {
  it('shows simultaneous requirements, scope and curated packages without offering setup', () => {
    const handlers = props()
    render(<EnvironmentHelpDialog {...handlers} />)
    expect(
      screen.getByRole('dialog', { name: 'Diagnóstico de SakuraRO' }),
    ).toBeTruthy()
    expect(screen.getByText('Herramienta cabextract')).toBeTruthy()
    expect(screen.getByText('Herramienta unzip')).toBeTruthy()
    expect(screen.getAllByText('Para preparar o rearmar')).toHaveLength(2)
    expect(screen.getByText('cabextract · unzip')).toBeTruthy()
    expect(
      screen.queryByRole('button', { name: 'Preparar entorno' }),
    ).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Volver a comprobar' }))
    expect(handlers.onRecheck).toHaveBeenCalledOnce()
    expect(handlers.onPrepare).not.toHaveBeenCalled()
  })

  it('keeps playing available when only repair tools are missing', () => {
    render(
      <EnvironmentHelpDialog
        {...props({ ...missingTools, launch: 'ready' })}
      />,
    )
    expect(screen.getByText(/Puedes jugar/)).toBeTruthy()
    expect(screen.getByText('Requisitos para reparar')).toBeTruthy()
    expect(preparationFailure({ ...missingTools, launch: 'ready' })).toBeNull()
  })

  it('offers preparation after a successful recheck and disables actions while busy', () => {
    const handlers = props({
      ...missingTools,
      launch: 'needsPreparation',
      preparationAvailable: true,
      actions: [{ kind: 'recheck' }, { kind: 'prepare' }],
      issues: [],
    })
    const { rerender } = render(<EnvironmentHelpDialog {...handlers} />)
    fireEvent.click(screen.getByRole('button', { name: 'Preparar entorno' }))
    expect(handlers.onPrepare).toHaveBeenCalledOnce()
    rerender(<EnvironmentHelpDialog {...handlers} busy />)
    expect(
      (
        screen.getByRole('button', {
          name: 'Preparar entorno',
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true)
    expect(
      (
        screen.getByRole('button', {
          name: 'Volver a comprobar',
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true)
  })

  it('contains keyboard focus and returns it when closed', () => {
    const trigger = document.createElement('button')
    document.body.append(trigger)
    trigger.focus()
    const handlers = props()
    const { unmount } = render(<EnvironmentHelpDialog {...handlers} />)
    expect(document.activeElement).toBe(
      screen.getByRole('button', { name: 'Cerrar' }),
    )
    const last = screen.getByRole('button', { name: 'Volver a comprobar' })
    last.focus()
    fireEvent.keyDown(document, { key: 'Tab' })
    expect(document.activeElement).toBe(
      screen.getByRole('button', { name: 'Copiar paquetes' }),
    )
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(handlers.onClose).toHaveBeenCalledOnce()
    unmount()
    expect(document.activeElement).toBe(trigger)
    trigger.remove()
  })

  it('allows read-only recheck but blocks preparation with active clients', () => {
    const handlers = props({
      ...missingTools,
      launch: 'needsPreparation',
      preparationAvailable: true,
      actions: [{ kind: 'recheck' }, { kind: 'prepare' }],
      issues: [],
    })
    render(<EnvironmentHelpDialog {...handlers} clientsActive />)
    const prepare = screen.getByRole('button', {
      name: 'Preparar entorno',
    }) as HTMLButtonElement
    expect(prepare.disabled).toBe(true)
    fireEvent.click(prepare)
    expect(handlers.onPrepare).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Volver a comprobar' }))
    expect(handlers.onRecheck).toHaveBeenCalledOnce()
  })

  it('handles clipboard failure without claiming packages were copied', async () => {
    const writeText = vi
      .fn()
      .mockRejectedValue(new Error('clipboard unavailable'))
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    })
    render(<EnvironmentHelpDialog {...props()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Copiar paquetes' }))
    expect(await screen.findByText(/No se pudo copiar/)).toBeTruthy()
    expect(writeText).toHaveBeenCalledWith('cabextract unzip')
  })

  it('preserves all failures in the launch summary and exposes technical detail on demand', () => {
    expect(preparationFailure(missingTools)).toContain(
      'Herramienta cabextract · Herramienta unzip',
    )
    render(<EnvironmentHelpDialog {...props()} />)
    const details = screen
      .getByText('host-cabextract · Falta cabextract')
      .closest('details')
    expect(details?.open).toBe(false)
  })

  it('presents compatibility warnings as recommendations without suggesting a runner change', () => {
    render(
      <EnvironmentHelpDialog
        {...props({
          ...missingTools,
          launch: 'ready',
          preparationAvailable: true,
          actions: [{ kind: 'recheck' }],
          issues: [
            {
              code: 'gepard-runner',
              domain: 'compatibility',
              severity: 'warning',
              affects: ['feature'],
              title: 'Compatibilidad de Gepard',
              message: 'Hash desconocido',
              remediation: 'Conserva un prefix separado al probar runners',
            },
          ],
        })}
      />,
    )
    expect(screen.getByText('Recomendación de compatibilidad')).toBeTruthy()
    expect(screen.getByText('Entorno listo')).toBeTruthy()
    expect(
      screen.queryByRole('button', { name: 'Preparar entorno' }),
    ).toBeNull()
  })
})
