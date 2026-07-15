import type {
  GearSwitchConfig,
  GearSwitchRule,
  SpammerConfig,
} from '../../shared/types'
import {
  DEFAULT_GEAR_SWITCH_CONFIG,
  DEFAULT_SPAMMER_CONFIG,
  GEAR_SWITCH_MAX_DELAY_MS,
  GEAR_SWITCH_MIN_DELAY_MS,
  SPAMMER_KEYS,
} from '../../shared/constants'

export type GearKeyField = 'atkKeys' | 'defKeys'

interface LegacyGearSwitchConfig {
  enabled?: boolean
  switchDelayMs?: number
  triggerKeys?: string[]
  atkKeys?: string[]
  defKeys?: string[]
}

export type GearSwitchInput = Partial<GearSwitchConfig> & LegacyGearSwitchConfig

type SpammerConfigInput = Omit<Partial<SpammerConfig>, 'gearSwitch'> & {
  gearSwitch?: GearSwitchInput
}

const KEY_ORDER = new Map<string, number>(
  SPAMMER_KEYS.map((key, index) => [key, index]),
)
const SPAMMER_KEY_SET = new Set<string>(SPAMMER_KEYS)

function normalizeKeys(keys: string[]): string[] {
  const seen = new Set<string>()
  const out: string[] = []
  for (const raw of keys) {
    const key = raw.trim().toUpperCase()
    if (!SPAMMER_KEY_SET.has(key) || seen.has(key)) continue
    seen.add(key)
    out.push(key)
  }
  return out.sort((a, b) => (KEY_ORDER.get(a) ?? 99) - (KEY_ORDER.get(b) ?? 99))
}

function clampGearDelay(ms?: number): number {
  const value = Number(ms)
  if (!Number.isFinite(value)) return DEFAULT_GEAR_SWITCH_CONFIG.switchDelayMs
  return Math.min(
    GEAR_SWITCH_MAX_DELAY_MS,
    Math.max(GEAR_SWITCH_MIN_DELAY_MS, Math.round(value)),
  )
}

function normalizeRule(rule?: Partial<GearSwitchRule>): GearSwitchRule | null {
  const trigger = (rule?.trigger ?? '').trim().toUpperCase()
  if (!SPAMMER_KEY_SET.has(trigger)) return null
  return {
    trigger,
    atkKeys: normalizeKeys(rule?.atkKeys ?? []),
    defKeys: normalizeKeys(rule?.defKeys ?? []),
  }
}

export function mergeGearSwitchConfig(
  config?: GearSwitchInput,
  allowedTriggers?: string[],
): GearSwitchConfig {
  const allowed = allowedTriggers
    ? new Set(normalizeKeys(allowedTriggers))
    : null
  const seen = new Set<string>()
  const rules: GearSwitchRule[] = []
  const legacyConfigured =
    !!config &&
    ((config.triggerKeys?.length ?? 0) > 0 ||
      (config.atkKeys?.length ?? 0) > 0 ||
      (config.defKeys?.length ?? 0) > 0)
  const legacyTriggers = config?.triggerKeys?.length
    ? config.triggerKeys
    : (allowedTriggers ?? [])
  const rawRules =
    config?.rules?.length || !legacyConfigured
      ? (config?.rules ?? [])
      : legacyTriggers.map((trigger) => ({
          trigger,
          atkKeys: config?.atkKeys ?? [],
          defKeys: config?.defKeys ?? [],
        }))

  for (const raw of rawRules) {
    const rule = normalizeRule(raw)
    if (
      !rule ||
      seen.has(rule.trigger) ||
      (allowed && !allowed.has(rule.trigger))
    ) {
      continue
    }
    seen.add(rule.trigger)
    rules.push(rule)
  }
  return {
    enabled: config?.enabled ?? DEFAULT_GEAR_SWITCH_CONFIG.enabled,
    switchDelayMs: clampGearDelay(config?.switchDelayMs),
    rules,
  }
}

export function makeGearRule(trigger: string): GearSwitchRule {
  return { trigger: trigger.toUpperCase(), atkKeys: [], defKeys: [] }
}

export function addGearRule(
  gear: GearSwitchConfig,
  trigger: string,
): GearSwitchConfig {
  const normalized = trigger.trim().toUpperCase()
  if (
    !SPAMMER_KEY_SET.has(normalized) ||
    gear.rules.some((r) => r.trigger === normalized)
  ) {
    return gear
  }
  return { ...gear, rules: [...gear.rules, makeGearRule(normalized)] }
}

export function removeGearRule(
  gear: GearSwitchConfig,
  trigger: string,
): GearSwitchConfig {
  return { ...gear, rules: gear.rules.filter((r) => r.trigger !== trigger) }
}

export function updateGearRule(
  gear: GearSwitchConfig,
  trigger: string,
  patch: Partial<GearSwitchRule>,
): GearSwitchConfig {
  return {
    ...gear,
    rules: gear.rules.map((r) =>
      r.trigger === trigger ? { ...r, ...patch } : r,
    ),
  }
}

export function toggleGearRuleKey(
  gear: GearSwitchConfig,
  trigger: string,
  field: GearKeyField,
  key: string,
): GearSwitchConfig {
  const normalized = key.toUpperCase()
  return updateGearRule(gear, trigger, {
    [field]: (() => {
      const rule = gear.rules.find((r) => r.trigger === trigger)
      const current = rule?.[field] ?? []
      return current.includes(normalized)
        ? current.filter((k) => k !== normalized)
        : normalizeKeys([...current, normalized])
    })(),
  })
}

export function mergeSpammerConfig(config?: SpammerConfigInput): SpammerConfig {
  const keys = normalizeKeys(
    config?.keys?.length ? [...config.keys] : [...DEFAULT_SPAMMER_CONFIG.keys],
  )
  return {
    ...DEFAULT_SPAMMER_CONFIG,
    ...config,
    keys,
    gearSwitch: mergeGearSwitchConfig(config?.gearSwitch, keys),
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

export function toggleSpammerKey(
  config: SpammerConfig,
  key: string,
): SpammerConfig {
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
