import { describe, expect, it } from 'vitest'
import {
  advancedHasIssue,
  resolveAudioDotStatus,
  resolveDotStatus,
} from './advanced.logic'

describe('resolveDotStatus', () => {
  it('verde cuando ok sin aviso', () => {
    expect(resolveDotStatus(true, null)).toBe('ok')
  })

  it('amarillo cuando ok con aviso parcial', () => {
    expect(resolveDotStatus(true, 'pendiente')).toBe('warning')
  })

  it('rojo cuando falla', () => {
    expect(resolveDotStatus(false, 'instalar paquete')).toBe('error')
  })
})

describe('resolveAudioDotStatus', () => {
  it('rojo sin backend de audio', () => {
    expect(resolveAudioDotStatus(false, 'sin libs')).toBe('error')
  })

  it('amarillo solo con aviso real', () => {
    expect(resolveAudioDotStatus(true, null)).toBe('ok')
    expect(resolveAudioDotStatus(true, 'problema detectado')).toBe('warning')
  })
})

describe('advancedHasIssue', () => {
  it('sin problemas cuando todo verde', () => {
    expect(
      advancedHasIssue({
        audioOk: true,
        audioDriver: 'pulse',
        audioStack: 'pipewire',
        audioWarning: null,
        inputGroupOk: true,
        inputGroupWarning: null,
        autopotInputOk: true,
        autopotInputWarning: null,
        prefixOk: true,
        prefixWarning: null,
        dxvkOk: true,
        dxvkWarning: null,
      }),
    ).toBe(false)
  })

  it('detecta dxvk pendiente como aviso', () => {
    expect(
      advancedHasIssue({
        audioOk: true,
        audioDriver: 'pulse',
        audioStack: 'pipewire',
        audioWarning: null,
        inputGroupOk: false,
        inputGroupWarning: 'usermod',
        autopotInputOk: false,
        autopotInputWarning: 'falta ydotool',
        prefixOk: false,
        prefixWarning: 'configura',
        dxvkOk: true,
        dxvkWarning: 'tras prefix',
      }),
    ).toBe(true)
  })

  it('ignora autopot e input group para el aviso del panel', () => {
    expect(
      advancedHasIssue({
        audioOk: true,
        audioDriver: 'pulse',
        audioStack: 'pipewire',
        audioWarning: null,
        inputGroupOk: false,
        inputGroupWarning: 'usermod',
        autopotInputOk: false,
        autopotInputWarning: 'falta ydotool',
        prefixOk: true,
        prefixWarning: null,
        dxvkOk: true,
        dxvkWarning: null,
      }),
    ).toBe(false)
  })
})
