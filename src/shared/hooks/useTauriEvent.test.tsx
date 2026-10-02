// @vitest-environment jsdom
import { StrictMode, type PropsWithChildren } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import { deferred } from '../../test/deferred'
import { useTauriEvent } from './useTauriEvent'

const { listen } = vi.hoisted(() => ({ listen: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen }))

it('uses the latest handler without resubscribing', async () => {
  let emit: ((event: { payload: number }) => void) | undefined
  const unlisten = vi.fn()
  listen
    .mockReset()
    .mockImplementation(
      async (_event: string, handler: (event: { payload: number }) => void) => {
        emit = handler
        return unlisten
      },
    )
  const first = vi.fn()
  const second = vi.fn()
  const { result, rerender, unmount } = renderHook(
    ({ handler }) => useTauriEvent<number>('status', handler),
    { initialProps: { handler: first } },
  )
  await waitFor(() => expect(result.current).toBe(true))
  act(() => emit?.({ payload: 1 }))
  rerender({ handler: second })
  expect(result.current).toBe(true)
  act(() => emit?.({ payload: 2 }))
  expect(first).toHaveBeenCalledWith(1)
  expect(second).toHaveBeenCalledWith(2)
  expect(listen).toHaveBeenCalledTimes(1)
  unmount()
  expect(unlisten).toHaveBeenCalledOnce()
})

it('ignores late events from a cleaned up StrictMode subscription before unlisten resolves', async () => {
  const first = deferred<() => void>()
  const second = deferred<() => void>()
  listen
    .mockReset()
    .mockReturnValueOnce(first.promise)
    .mockReturnValueOnce(second.promise)
  const handler = vi.fn()
  const wrapper = ({ children }: PropsWithChildren) => (
    <StrictMode>{children}</StrictMode>
  )
  const hook = renderHook(() => useTauriEvent('exit', handler), { wrapper })
  const oldCallback = listen.mock.calls[0][1] as (event: {
    payload: number
  }) => void
  const currentCallback = listen.mock.calls[1][1] as (event: {
    payload: number
  }) => void
  oldCallback({ payload: 1 })
  currentCallback({ payload: 2 })
  expect(handler.mock.calls).toEqual([[2]])
  hook.unmount()
  currentCallback({ payload: 3 })
  const oldUnlisten = vi.fn()
  const currentUnlisten = vi.fn()
  await act(async () => {
    first.resolve(oldUnlisten)
    second.resolve(currentUnlisten)
  })
  expect(handler.mock.calls).toEqual([[2]])
  expect(oldUnlisten).toHaveBeenCalledTimes(1)
  expect(currentUnlisten).toHaveBeenCalledTimes(1)
})
