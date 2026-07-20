// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { LaunchFieldsModal } from './LaunchFieldsModal'

afterEach(cleanup)

describe('LaunchFieldsModal', () => {
  it('does not treat Object.prototype keys as submitted values', () => {
    const onSubmit = vi.fn()
    render(
      <LaunchFieldsModal
        serverName="SakuraRO"
        fields={['constructor']}
        onCancel={vi.fn()}
        onSubmit={onSubmit}
      />,
    )

    const submit = screen.getByRole('button', { name: 'Continuar' })
    expect(submit).toBeDisabled()

    fireEvent.change(screen.getByLabelText('constructor'), {
      target: { value: 'ephemeral-value' },
    })
    expect(submit).toBeEnabled()
    fireEvent.click(submit)

    expect(onSubmit).toHaveBeenCalledWith({
      constructor: 'ephemeral-value',
    })
  })
})
