import { describe, expect, it } from 'vitest'
import {
  formatSpammerKeys,
  mergeSpammerConfig,
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
      mergeSpammerConfig({ keys: ['f2', 'F1', 'F2', 'Q'] }),
    ).toMatchObject({
      keys: ['F1', 'F2'],
    })
  })
})

describe('toggleSpammerKey', () => {
  it('adds and removes keys', () => {
    const base = mergeSpammerConfig({ keys: ['F1'] })
    const withF2 = toggleSpammerKey(base, 'F2')
    expect(withF2.keys).toEqual(['F1', 'F2'])
    expect(toggleSpammerKey(withF2, 'F1').keys).toEqual(['F2'])
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
