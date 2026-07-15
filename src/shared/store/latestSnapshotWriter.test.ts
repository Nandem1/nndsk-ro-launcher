import { describe, expect, it, vi } from 'vitest'
import { deferred } from '../../test/deferred'
import { LatestSnapshotWriter } from './latestSnapshotWriter'

describe('LatestSnapshotWriter', () => {
  it('keeps one write in flight and coalesces pending snapshots', async () => {
    const first = deferred<void>()
    const save = vi
      .fn<(value: number) => Promise<void>>()
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce()
    const writer = new LatestSnapshotWriter(save)

    const one = writer.write(1)
    const two = writer.write(2)
    const three = writer.write(3)

    expect(save).toHaveBeenCalledTimes(1)
    first.resolve()
    await Promise.all([one, two, three])

    expect(save.mock.calls).toEqual([[1], [3]])
  })

  it('recovers after a failed write', async () => {
    const save = vi
      .fn<(value: number) => Promise<void>>()
      .mockRejectedValueOnce(new Error('disk full'))
      .mockResolvedValueOnce()
    const writer = new LatestSnapshotWriter(save)

    await expect(writer.write(1)).rejects.toThrow('disk full')
    await expect(writer.write(2)).resolves.toBeUndefined()
    expect(save.mock.calls).toEqual([[1], [2]])
  })
})
