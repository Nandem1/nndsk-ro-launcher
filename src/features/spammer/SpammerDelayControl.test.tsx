// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { SpammerDelayControl } from './SpammerDelayControl'

afterEach(cleanup)

describe('SpammerDelayControl', () => {
  it('commits only the final pointer value', () => {
    const onCommit = vi.fn()
    render(
      <SpammerDelayControl
        configuredDelayMs={10}
        disabled={false}
        onCommit={onCommit}
      />,
    )
    const slider = screen.getByRole('slider', { name: 'Delay del spammer' })

    fireEvent.change(slider, { target: { value: '11' } })
    fireEvent.change(slider, { target: { value: '17' } })
    fireEvent.change(slider, { target: { value: '20' } })

    expect(onCommit).not.toHaveBeenCalled()
    fireEvent.pointerUp(slider)
    expect(onCommit).toHaveBeenCalledTimes(1)
    expect(onCommit).toHaveBeenCalledWith(20)

    fireEvent.blur(slider)
    expect(onCommit).toHaveBeenCalledTimes(1)
  })

  it('commits a keyboard adjustment on key-up', () => {
    const onCommit = vi.fn()
    render(
      <SpammerDelayControl
        configuredDelayMs={20}
        disabled={false}
        onCommit={onCommit}
      />,
    )
    const slider = screen.getByRole('slider', { name: 'Delay del spammer' })

    fireEvent.change(slider, { target: { value: '21' } })
    fireEvent.keyUp(slider, { key: 'ArrowRight' })

    expect(onCommit).toHaveBeenCalledOnce()
    expect(onCommit).toHaveBeenCalledWith(21)
  })
})
