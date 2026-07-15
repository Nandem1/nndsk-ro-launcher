// @vitest-environment jsdom

import { act, renderHook, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { useTauriEvent } from './useTauriEvent'

const mocks = vi.hoisted(() => ({ listen: vi.fn() }))

vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }))

describe('useTauriEvent', () => {
  it('uses the latest handler without resubscribing', async () => {
    let emit: ((event: { payload: number }) => void) | undefined
    const unlisten = vi.fn()
    mocks.listen.mockImplementation(
      async (_event: string, handler: (event: { payload: number }) => void) => {
        emit = handler
        return unlisten
      },
    )
    const first = vi.fn()
    const second = vi.fn()

    const { rerender, unmount } = renderHook(
      ({ handler }) => useTauriEvent<number>('status', handler),
      { initialProps: { handler: first } },
    )
    await waitFor(() => expect(emit).toBeDefined())

    act(() => emit?.({ payload: 1 }))
    rerender({ handler: second })
    act(() => emit?.({ payload: 2 }))

    expect(first).toHaveBeenCalledWith(1)
    expect(second).toHaveBeenCalledWith(2)
    expect(mocks.listen).toHaveBeenCalledTimes(1)

    unmount()
    expect(unlisten).toHaveBeenCalledOnce()
  })
})
