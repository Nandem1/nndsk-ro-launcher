import type { RunnerInfo } from '../../shared/types'

export interface RunnerResolution {
  path: string
  /** Persistir en settings.json cuando una resolución futura lo requiera. */
  persist: boolean
}

/** Decide el runner tras cargar la lista disponible. */
export function resolveRunnerAfterLoad(
  current: string,
  runners: RunnerInfo[],
): RunnerResolution | null {
  if (runners.length === 0) return null

  const managed = runners[0]
  return { path: managed.path, persist: current !== managed.path }
}
