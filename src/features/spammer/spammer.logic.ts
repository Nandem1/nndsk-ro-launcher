import type { SpammerConfig } from '../../shared/types'
import { DEFAULT_SPAMMER_CONFIG, POT_KEYS } from '../../shared/constants'

const KEY_ORDER = new Map<string, number>(POT_KEYS.map((key, index) => [key, index]))
const POT_KEY_SET = new Set<string>(POT_KEYS)

function normalizeKeys(keys: string[]): string[] {
  const seen = new Set<string>()
  const out: string[] = []
  for (const raw of keys) {
    const key = raw.trim().toUpperCase()
    if (!POT_KEY_SET.has(key) || seen.has(key)) continue
    seen.add(key)
    out.push(key)
  }
  return out.sort(
    (a, b) => (KEY_ORDER.get(a) ?? 99) - (KEY_ORDER.get(b) ?? 99),
  )
}

export function mergeSpammerConfig(config?: Partial<SpammerConfig>): SpammerConfig {
  const keys = normalizeKeys(
    config?.keys?.length ? [...config.keys] : [...DEFAULT_SPAMMER_CONFIG.keys],
  )
  return {
    ...DEFAULT_SPAMMER_CONFIG,
    ...config,
    keys,
    enabled: false,
  }
}

export type PersistedSpammerPatch = Partial<Omit<SpammerConfig, 'enabled'>>

export function withSpammerPatch(
  config: SpammerConfig,
  patch: PersistedSpammerPatch,
): SpammerConfig {
  const merged = { ...config, ...patch }
  return mergeSpammerConfig(merged)
}

export function toggleSpammerKey(config: SpammerConfig, key: string): SpammerConfig {
  const normalized = key.toUpperCase()
  const has = config.keys.includes(normalized)
  const keys = has
    ? config.keys.filter((k) => k !== normalized)
    : normalizeKeys([...config.keys, normalized])
  return { ...config, keys }
}

export function formatSpammerKeys(keys: string[]): string {
  if (keys.length === 0) return '—'
  return keys.join(' · ')
}
