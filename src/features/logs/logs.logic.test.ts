import { describe, expect, it } from 'vitest'
import { countLogErrors, isLogError, logLineClass } from './logs.logic'

describe('isLogError', () => {
  it('cuenta errores reales', () => {
    expect(isLogError('ERROR no se pudo abrir el cliente')).toBe(true)
    expect(isLogError('[AutoPot] Config falló: permiso denegado')).toBe(true)
  })

  it('no cuenta probe fallido de AutoPot como error accionable', () => {
    expect(isLogError('[AutoPot] Probe falló: reintentando')).toBe(false)
  })
})

describe('countLogErrors', () => {
  it('ignora warnings y suma errores', () => {
    expect(
      countLogErrors([
        'warn: audio fallback',
        '[AutoPot] Probe falló: reintentando',
        'ERROR prefix inválido',
      ]),
    ).toBe(1)
  })
})

describe('logLineClass', () => {
  it('asigna clases según severidad o fuente', () => {
    expect(logLineClass('ERROR prefix inválido')).toBe('text-bad-bright')
    expect(logLineClass('warn: audio fallback')).toBe('text-accent-bright')
    expect(logLineClass('[AutoPot] Probe OK')).toBe('text-ok-bright/80')
    expect(logLineClass('[AutoPot] HP/SP listo')).toBe('text-info-bright/90')
    expect(logLineClass('línea normal')).toBe('text-ink-soft')
  })
})
