// @vitest-environment jsdom

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { mergeSpammerConfig } from './spammer.logic'
import { SpammerKeyboard } from './SpammerKeyboard'

describe('SpammerKeyboard', () => {
  it('emits the complete key set when a key is toggled', () => {
    const onKeysChange = vi.fn()
    render(
      <SpammerKeyboard
        config={mergeSpammerConfig()}
        armed={false}
        available={true}
        disabled={false}
        onKeysChange={onKeysChange}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'F2' }))

    expect(onKeysChange).toHaveBeenCalledWith(['F1', 'F2'])
  })
})
