import { describe, expect, it } from 'vitest'
import {
  advancedHasIssue,
  dxvkHintFromDeps,
  resolveAudioDotStatus,
  resolveDotStatus,
} from './advanced.logic'
import type { DependencyStatus } from '../../shared/types'
import type { AdvancedDepsStatus } from '../../shared/types'

const healthyStatus: AdvancedDepsStatus = {
  audioOk: true,
  audioDriver: 'pulse',
  audioStack: 'pipewire',
  audioWarning: null,
  inputGroupOk: true,
  inputGroupWarning: null,
  uinputInputOk: true,
  uinputInputWarning: null,
  prefixOk: true,
  prefixWarning: null,
  dxvkOk: true,
  dxvk: true,
  dxvkWarning: null,
  runnerKind: 'proton',
  runnerOk: true,
  runnerWarning: null,
  prefixPath: '/tmp/ro-launcher-test-prefix',
  prefixScope: 'isolated',
  prefixManaged: true,
  readyToLaunch: true,
  canSetup: true,
  canReset: true,
  checks: [],
}

describe('dxvkHintFromDeps', () => {
  const baseDeps = (): DependencyStatus => ({
    wine: true,
    winetricks: true,
    dxvk: true,
    prefixConfigured: true,
    audioOk: true,
    audioDriver: 'pulse',
    audioStack: 'pipewire',
    audioWarning: null,
    inputGroupOk: true,
    inputGroupWarning: null,
    uinputInputOk: true,
    uinputInputWarning: null,
    prefixOk: true,
    prefixWarning: null,
    dxvkOk: true,
    dxvkWarning: null,
    runnerKind: 'proton',
    runnerOk: true,
    runnerWarning: null,
    prefixPath: '/tmp/prefix',
    prefixScope: 'isolated',
    prefixManaged: true,
    readyToLaunch: true,
    canSetup: true,
    canReset: true,
    checks: [],
  })

  it('prioriza el aviso sobre el plan', () => {
    expect(
      dxvkHintFromDeps({
        ...baseDeps(),
        dxvkWarning: 'pendiente',
        runtimePlan: {
          planId: 'abc',
          selectionSource: 'productDefault',
          graphicsProfile: 'dxvk',
          dxvkProvider: 'runnerOwned',
          dxvkComponentId: 'runner/dxvk',
          overlayVerified: false,
        },
      }),
    ).toBe('pendiente')
  })

  it('usa el plan cuando no hay aviso', () => {
    expect(
      dxvkHintFromDeps({
        ...baseDeps(),
        runtimePlan: {
          planId: 'abc',
          selectionSource: 'productDefault',
          graphicsProfile: 'dxvk',
          dxvkProvider: 'managedPrefix',
          dxvkComponentId: 'dxvk-2.6.2',
          overlayVerified: false,
        },
      }),
    ).toBe('managedPrefix · dxvk-2.6.2')
  })
})

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
    expect(advancedHasIssue(healthyStatus)).toBe(false)
  })

  it('detecta dxvk pendiente como aviso', () => {
    expect(
      advancedHasIssue({
        ...healthyStatus,
        inputGroupOk: false,
        inputGroupWarning: 'usermod',
        uinputInputOk: false,
        uinputInputWarning: 'falta uinput',
        prefixOk: false,
        prefixWarning: 'configura',
        dxvkWarning: 'tras prefix',
      }),
    ).toBe(true)
  })

  it('detecta uinput no disponible como problema de producción', () => {
    expect(
      advancedHasIssue({
        ...healthyStatus,
        uinputInputOk: false,
        uinputInputWarning: 'falta /dev/uinput',
      }),
    ).toBe(true)
  })

  it('ignora input group aislado para el aviso del panel', () => {
    expect(
      advancedHasIssue({
        ...healthyStatus,
        inputGroupOk: false,
        inputGroupWarning: 'usermod',
      }),
    ).toBe(false)
  })
})
