import {
  LEGACY_DEFAULT_WINE,
  PREFERRED_PROTON_ID,
} from '../../shared/constants'
import type { RunnerInfo } from '../../shared/types'

export interface RunnerResolution {
  path: string
  /** Persistir en settings.json (p. ej. migración desde wine legacy). */
  persist: boolean
}

/** Decide el runner tras cargar la lista disponible. */
export function resolveRunnerAfterLoad(
  current: string,
  runners: RunnerInfo[],
): RunnerResolution | null {
  if (runners.length === 0) return null

  const preferred = runners.find((r) => r.id === PREFERRED_PROTON_ID)
  const fallback = runners[0]

  if (!current) {
    return { path: preferred?.path ?? fallback.path, persist: false }
  }

  if (current === LEGACY_DEFAULT_WINE && preferred) {
    return { path: preferred.path, persist: true }
  }

  return { path: current, persist: false }
}
