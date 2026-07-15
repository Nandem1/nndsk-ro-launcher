interface Waiter {
  revision: number
  resolve: () => void
  reject: (reason?: unknown) => void
}

/** Serializes writes and coalesces rapid updates into the newest snapshot. */
export class LatestSnapshotWriter<T> {
  private revision = 0
  private persistedRevision = 0
  private latest: T | undefined
  private running: Promise<void> | null = null
  private waiters: Waiter[] = []

  constructor(private readonly save: (snapshot: T) => Promise<void>) {}

  write(snapshot: T): Promise<void> {
    const revision = ++this.revision
    this.latest = snapshot

    const promise = new Promise<void>((resolve, reject) => {
      this.waiters.push({ revision, resolve, reject })
    })
    this.startDrain()
    return promise
  }

  private startDrain(): void {
    if (this.running) return
    this.running = this.drain().finally(() => {
      this.running = null
      if (this.latest && this.persistedRevision < this.revision) {
        this.startDrain()
      }
    })
  }

  private async drain(): Promise<void> {
    while (this.persistedRevision < this.revision) {
      const targetRevision = this.revision
      const snapshot = this.latest
      if (!snapshot) return

      try {
        await this.save(snapshot)
      } catch (error) {
        this.persistedRevision = this.revision
        this.rejectWaiters(error)
        return
      }

      this.persistedRevision = targetRevision
      this.resolveWaiters(targetRevision)
    }
  }

  private resolveWaiters(revision: number): void {
    const pending: Waiter[] = []
    for (const waiter of this.waiters) {
      if (waiter.revision <= revision) waiter.resolve()
      else pending.push(waiter)
    }
    this.waiters = pending
  }

  private rejectWaiters(error: unknown): void {
    const waiters = this.waiters
    this.waiters = []
    for (const waiter of waiters) waiter.reject(error)
  }
}
