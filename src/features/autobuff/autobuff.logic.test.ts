import { describe, expect, it } from 'vitest'
import { mergeAutobuffConfig } from './autobuff.logic'

describe('mergeAutobuffConfig', () => {
  it('keeps one rule per status ID and removes invalid placeholders', () => {
    const config = mergeAutobuffConfig({
      rules: [
        { id: 'agi', label: 'AGI', statusId: 12, key: 'F1', cooldownMs: 1000, priority: 0, enabled: true },
        { id: 'agi-scroll', label: 'AGI scroll', statusId: 12, key: 'F2', cooldownMs: 1000, priority: 1, enabled: true },
        { id: 'empty', label: 'Empty', statusId: 0, key: 'F3', cooldownMs: 1000, priority: 2, enabled: false },
      ],
    })
    expect(config.rules).toHaveLength(1)
    expect(config.rules[0].id).toBe('agi')
  })
})
