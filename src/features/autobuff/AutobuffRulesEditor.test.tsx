// @vitest-environment jsdom

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { AutobuffRulesEditor } from './AutobuffRulesEditor'

describe('AutobuffRulesEditor', () => {
  it('creates the same default manual rule', () => {
    const onChange = vi.fn()
    render(
      <AutobuffRulesEditor rules={[]} disabled={false} onChange={onChange} />,
    )

    fireEvent.click(screen.getByRole('button', { name: '+ Manual' }))

    expect(onChange).toHaveBeenCalledWith([
      expect.objectContaining({
        label: 'Nuevo buff',
        statusId: 1,
        key: 'F1',
        cooldownMs: 1000,
        priority: 100,
        enabled: false,
      }),
    ])
  })
})
