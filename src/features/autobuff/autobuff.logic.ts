import type { AutobuffConfig } from '../../shared/types'
import { DEFAULT_AUTOBUFF_CONFIG } from '../../shared/constants'

export function mergeAutobuffConfig(config?: Partial<AutobuffConfig>): AutobuffConfig {
  const seenStatusIds = new Set<number>()
  const rules = (config?.rules ?? []).filter((rule) => {
    if (rule.statusId <= 0 || seenStatusIds.has(rule.statusId)) return false
    seenStatusIds.add(rule.statusId)
    return true
  })
  return { ...DEFAULT_AUTOBUFF_CONFIG, ...config, rules, enabled: false }
}

export type PersistedAutobuffPatch = Partial<Omit<AutobuffConfig, 'enabled'>>

export function withAutobuffPatch(config: AutobuffConfig, patch: PersistedAutobuffPatch): AutobuffConfig {
  return mergeAutobuffConfig({ ...config, ...patch })
}
