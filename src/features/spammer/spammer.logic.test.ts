import { describe, expect, it } from 'vitest'
import { SPAMMER_KEYS } from '../../shared/constants'
import {
  addGearRule,
  formatSpammerKeys,
  mergeGearSwitchConfig,
  mergeSpammerConfig,
  removeGearRule,
  toggleGearRuleKey,
  toggleSpammerKey,
  withSpammerPatch,
} from './spammer.logic'

describe('mergeSpammerConfig', () => {
  it('defaults keys to F1 when missing', () => {
    expect(mergeSpammerConfig({ delayMs: 20 })).toMatchObject({
      keys: ['F1'],
      delayMs: 20,
      enabled: false,
    })
  })

  it('normalizes and deduplicates keys', () => {
    expect(
      mergeSpammerConfig({ keys: ['z', 'f2', 'F1', 'F2', 'q', 'SPACE'] }),
    ).toMatchObject({
      keys: ['F1', 'F2', 'Q', 'Z'],
    })
  })

  it('keeps every supported key', () => {
    expect(mergeSpammerConfig({ keys: [...SPAMMER_KEYS] }).keys).toEqual(
      SPAMMER_KEYS,
    )
  })
})

describe('toggleSpammerKey', () => {
  it('adds and removes keys', () => {
    const base = mergeSpammerConfig({ keys: ['F1'] })
    const withF2 = toggleSpammerKey(base, 'F2')
    expect(withF2.keys).toEqual(['F1', 'F2'])
    const withLetter = toggleSpammerKey(withF2, 'p')
    expect(withLetter.keys).toEqual(['F1', 'F2', 'P'])
    expect(toggleSpammerKey(withLetter, 'F1').keys).toEqual(['F2', 'P'])
  })
})

describe('withSpammerPatch', () => {
  it('merges patch into persisted config', () => {
    const next = withSpammerPatch(mergeSpammerConfig(), { keys: ['F3', 'F4'] })
    expect(next.keys).toEqual(['F3', 'F4'])
  })
})

describe('formatSpammerKeys', () => {
  it('formats empty and non-empty lists', () => {
    expect(formatSpammerKeys([])).toBe('—')
    expect(formatSpammerKeys(['F1', 'F2'])).toBe('F1 · F2')
  })
})

describe('gear switch', () => {
  it('defaults gearSwitch when absent', () => {
    expect(mergeSpammerConfig({ keys: ['F3'] }).gearSwitch).toEqual({
      enabled: false,
      switchDelayMs: 50,
      rules: [],
    })
  })

  it('normalizes rules, drops duplicates and clamps switch delay', () => {
    const gear = mergeGearSwitchConfig({
      enabled: true,
      rules: [
        { trigger: 'f3', atkKeys: ['8', 'bogus'], defKeys: ['9'] },
        { trigger: 'F3', atkKeys: ['1'], defKeys: [] },
        { trigger: 'space', atkKeys: [], defKeys: [] },
      ],
      switchDelayMs: 5000,
    })
    expect(gear.rules).toEqual([
      { trigger: 'F3', atkKeys: ['8'], defKeys: ['9'] },
    ])
    expect(gear.switchDelayMs).toBe(300)
  })

  it('adds, edits and removes independent trigger rules', () => {
    const base = addGearRule(mergeGearSwitchConfig({ enabled: true }), 'f3')
    const withAtk = toggleGearRuleKey(base, 'F3', 'atkKeys', '8')
    const withDef = toggleGearRuleKey(withAtk, 'F3', 'defKeys', '9')
    const withSecondRule = addGearRule(withDef, 'F4')

    expect(withSecondRule.rules).toEqual([
      { trigger: 'F3', atkKeys: ['8'], defKeys: ['9'] },
      { trigger: 'F4', atkKeys: [], defKeys: [] },
    ])
    expect(removeGearRule(withSecondRule, 'F3').rules).toEqual([
      { trigger: 'F4', atkKeys: [], defKeys: [] },
    ])
  })

  it('propagates gearSwitch through withSpammerPatch', () => {
    const next = withSpammerPatch(mergeSpammerConfig({ keys: ['F3'] }), {
      gearSwitch: mergeGearSwitchConfig({
        enabled: true,
        rules: [{ trigger: 'F3', atkKeys: ['8'], defKeys: ['9'] }],
      }),
    })
    expect(next.gearSwitch.enabled).toBe(true)
    expect(next.gearSwitch.rules[0].atkKeys).toEqual(['8'])
  })

  it('drops rules whose trigger is no longer a spammer key', () => {
    const next = mergeSpammerConfig({
      keys: ['F4'],
      gearSwitch: {
        enabled: true,
        rules: [
          { trigger: 'F3', atkKeys: ['8'], defKeys: ['9'] },
          { trigger: 'F4', atkKeys: ['1'], defKeys: ['2'] },
        ],
      },
    })
    expect(next.gearSwitch.rules.map((rule) => rule.trigger)).toEqual(['F4'])
  })

  it('migrates the previous shared gear config into one rule per trigger', () => {
    const next = mergeSpammerConfig({
      keys: ['F3', 'F4'],
      gearSwitch: {
        enabled: true,
        triggerKeys: [],
        atkKeys: ['8'],
        defKeys: ['9'],
        switchDelayMs: 60,
      },
    })
    expect(next.gearSwitch.rules).toEqual([
      { trigger: 'F3', atkKeys: ['8'], defKeys: ['9'] },
      { trigger: 'F4', atkKeys: ['8'], defKeys: ['9'] },
    ])
  })
})
