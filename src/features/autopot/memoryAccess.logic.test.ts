import { describe, expect, it } from 'vitest'
import {
  memoryAccessAction,
  memoryAccessLabel,
  memoryAccessUsable,
} from './memoryAccess.logic'

describe('memoryAccess.logic', () => {
  it('marks only vm and proc mem as usable', () => {
    expect(memoryAccessUsable('processVmReadv')).toBe(true)
    expect(memoryAccessUsable('procMem')).toBe(true)
    expect(memoryAccessUsable('yamaDenied')).toBe(false)
  })

  it('uses closed Spanish labels', () => {
    expect(memoryAccessLabel('yamaDenied')).toBe('Permiso denegado por Yama')
    expect(memoryAccessLabel('outsideSupervisor')).toBe(
      'Proceso fuera del supervisor',
    )
  })

  it('suggests relaunch for supervisor and yama failures', () => {
    const action = memoryAccessAction('yamaDenied')
    expect(action).toContain('relanza el juego')
    expect(action).not.toMatch(/ptrace_scope/i)
  })
})
