// @vitest-environment jsdom

import { cleanup, fireEvent, render } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { SharpShootingEditor } from './SharpShootingEditor'

afterEach(cleanup)

describe('SharpShootingEditor', () => {
  it('enables the first spammer trigger by default', () => {
    const onChange = vi.fn()
    const view = render(
      <SharpShootingEditor
        spammerKeys={['F2', 'F3']}
        shiftMode={{ enabled: false, triggerKeys: [] }}
        disabled={false}
        onChange={onChange}
      />,
    )

    fireEvent.click(
      view.getByRole('button', {
        name: /Sharp Shooting \/ Focused Arrow Strike/i,
      }),
    )
    fireEvent.click(view.getByRole('switch'))

    expect(onChange).toHaveBeenCalledWith({
      enabled: true,
      triggerKeys: ['F2'],
    })
  })

  it('toggles a configured trigger', () => {
    const onChange = vi.fn()
    const view = render(
      <SharpShootingEditor
        spammerKeys={['F2', 'F3']}
        shiftMode={{ enabled: true, triggerKeys: ['F2'] }}
        disabled={false}
        onChange={onChange}
      />,
    )

    fireEvent.click(
      view.getByRole('button', {
        name: /Sharp Shooting \/ Focused Arrow Strike/i,
      }),
    )
    fireEvent.click(view.getByRole('button', { name: 'Usar F3 con Shift' }))

    expect(onChange).toHaveBeenCalledWith({
      enabled: true,
      triggerKeys: ['F2', 'F3'],
    })
  })
})
