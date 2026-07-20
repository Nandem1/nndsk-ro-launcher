// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { api } from '../../shared/api'
import { MemoryScannerModal, parseHp } from './MemoryScannerModal'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

describe('MemoryScannerModal', () => {
  it('validates positive u32 HP values', () => {
    expect(parseHp('13619')).toBe(13_619)
    expect(parseHp('0')).toBeNull()
    expect(parseHp('-1')).toBeNull()
    expect(parseHp('12.5')).toBeNull()
    expect(parseHp('4294967296')).toBeNull()
  })

  it('runs the initial scan, refines the same addresses and saves the result', async () => {
    vi.spyOn(api, 'beginAutopotMemoryScan').mockResolvedValue({
      pid: 42,
      candidateCount: 3,
      confirmed: null,
    })
    vi.spyOn(api, 'refineAutopotMemoryScan').mockResolvedValue({
      pid: 42,
      candidateCount: 1,
      confirmed: {
        hpBase: '0x0146F28C',
        currentHp: 13_430,
        maxHp: 20_000,
        currentSp: 700,
        maxSp: 900,
        statusBuffer: '0x0146F700',
      },
    })
    vi.spyOn(api, 'findAutopotNameAddress').mockResolvedValue({
      pid: 42,
      characterName: 'NombrePJ',
      nameAddress: '0x01471CD8',
    })
    vi.spyOn(api, 'cancelAutopotMemoryScan').mockResolvedValue()
    const onConfirm = vi.fn().mockResolvedValue(undefined)
    const onCancel = vi.fn()
    render(
      <MemoryScannerModal
        serverName="TestRO"
        onCancel={onCancel}
        onConfirm={onConfirm}
      />,
    )

    fireEvent.change(screen.getByLabelText('HP actual'), {
      target: { value: '13619' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Buscar' }))

    expect(await screen.findByText(/3 candidatos/)).toBeInTheDocument()
    fireEvent.change(screen.getByLabelText('Nuevo HP actual'), {
      target: { value: '13430' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Comparar' }))

    expect(await screen.findByLabelText('Nombre exacto')).toBeInTheDocument()
    fireEvent.change(screen.getByLabelText('Nombre exacto'), {
      target: { value: 'NombrePJ' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Buscar nombre' }))

    expect(await screen.findByText('0x0146F28C')).toBeInTheDocument()
    expect(screen.getByText('0x01471CD8')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Guardar direcciones' }))

    await vi.waitFor(() => {
      expect(onConfirm).toHaveBeenCalledWith('0x0146F28C', '0x01471CD8')
      expect(onCancel).toHaveBeenCalledOnce()
    })
  })

  it('does not refine while the HP is unchanged', async () => {
    vi.spyOn(api, 'beginAutopotMemoryScan').mockResolvedValue({
      pid: 42,
      candidateCount: 8,
      confirmed: null,
    })
    const refine = vi.spyOn(api, 'refineAutopotMemoryScan')
    render(
      <MemoryScannerModal
        serverName="TestRO"
        onCancel={vi.fn()}
        onConfirm={vi.fn()}
      />,
    )

    fireEvent.change(screen.getByLabelText('HP actual'), {
      target: { value: '13619' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Buscar' }))
    await screen.findByText(/8 candidatos/)
    fireEvent.change(screen.getByLabelText('Nuevo HP actual'), {
      target: { value: '13619' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Comparar' }))

    expect(await screen.findByText(/El HP no cambió/)).toBeInTheDocument()
    expect(refine).not.toHaveBeenCalled()
  })

  it('skips directly to the name when HP base is already configured', async () => {
    const beginHp = vi.spyOn(api, 'beginAutopotMemoryScan')
    vi.spyOn(api, 'findAutopotNameAddress').mockResolvedValue({
      pid: 42,
      characterName: 'NombrePJ',
      nameAddress: '0x01471CD8',
    })
    const onConfirm = vi.fn().mockResolvedValue(undefined)
    render(
      <MemoryScannerModal
        serverName="TestRO"
        existingHpBase="0x0146F28C"
        onCancel={vi.fn()}
        onConfirm={onConfirm}
      />,
    )

    expect(screen.queryByLabelText('HP actual')).not.toBeInTheDocument()
    fireEvent.change(screen.getByLabelText('Nombre exacto'), {
      target: { value: 'NombrePJ' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Buscar nombre' }))
    await screen.findByText('0x01471CD8')
    fireEvent.click(screen.getByRole('button', { name: 'Guardar direcciones' }))

    await vi.waitFor(() => {
      expect(beginHp).not.toHaveBeenCalled()
      expect(onConfirm).toHaveBeenCalledWith('0x0146F28C', '0x01471CD8')
    })
  })
})
