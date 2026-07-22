import { describe, expect, it } from 'vitest'
import { modeForStatus } from './uiMode.store'

describe('modeForStatus', () => {
  it('queda en prep mientras no hay juego', () => {
    expect(modeForStatus('idle')).toBe('prep')
    expect(modeForStatus('setting-up')).toBe('prep')
    expect(modeForStatus('error')).toBe('prep')
  })

  it('pasa a ingame desde launching', () => {
    expect(modeForStatus('launching')).toBe('ingame')
    expect(modeForStatus('idle', 2)).toBe('ingame')
  })
})
