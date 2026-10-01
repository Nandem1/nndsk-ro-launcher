// @vitest-environment jsdom

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

const minimize = vi.fn()
const close = vi.fn()

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ minimize, close }),
}))

import { WindowControls } from './WindowControls'

describe('WindowControls', () => {
  it('minimizes and closes the current window', () => {
    minimize.mockReset()
    close.mockReset()
    render(<WindowControls />)

    fireEvent.click(screen.getByRole('button', { name: 'Minimizar' }))
    fireEvent.click(screen.getByRole('button', { name: 'Cerrar' }))

    expect(minimize).toHaveBeenCalledOnce()
    expect(close).toHaveBeenCalledOnce()
  })
})
