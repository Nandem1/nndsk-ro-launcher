import type { AutopotConfig } from '../../shared/types'
import { DEFAULT_AUTOPOT_CONFIG } from '../../shared/constants'

export function mergeAutopotConfig(config?: AutopotConfig): AutopotConfig {
  return {
    ...DEFAULT_AUTOPOT_CONFIG,
    ...config,
    enabled: false,
  }
}

export type PersistedAutopotPatch = Partial<Omit<AutopotConfig, 'enabled'>>

export function withAutopotPatch(
  config: AutopotConfig,
  patch: PersistedAutopotPatch,
): AutopotConfig {
  return mergeAutopotConfig({ ...config, ...patch })
}

export function statPercent(cur: number, max: number): number {
  if (max <= 0) return 0
  return Math.min(100, Math.round((cur / max) * 100))
}
