import type { RunnerInfo } from '../../shared/types'
import { MANAGED_RUNTIME_ID } from '../../shared/constants'

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

  const preferred =
    runners.find((runner) => runner.id === MANAGED_RUNTIME_ID) ?? runners[0]
  if (current && runners.some((runner) => runner.path === current)) {
    return { path: current, persist: false }
  }
  return { path: preferred.path, persist: current !== preferred.path }
}
