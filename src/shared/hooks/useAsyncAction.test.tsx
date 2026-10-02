// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import { deferred } from '../../test/deferred'
import { useAsyncAction } from './useAsyncAction'

it('reserves synchronously so double clicks cannot execute a tool mutation twice', async () => {
  const pending = deferred<void>()
  const action = vi.fn(() => pending.promise)
  const { result } = renderHook(() => useAsyncAction<'tool'>())
  let first!: Promise<boolean>
  let second!: Promise<boolean>
  act(() => {
    first = result.current.run('tool', action)
    second = result.current.run('tool', action)
  })
  expect(action).toHaveBeenCalledTimes(1)
  expect(await second).toBe(false)
  await act(async () => {
    pending.resolve()
    await first
  })
  expect(result.current.busyKey).toBeNull()
})
