import { describe, expect, it } from 'vitest'
import {
  advancedHasIssue,
  benchmarksLabel,
  comparisonSummary,
  compatibilityLine,
  dxvkHintFromDeps,
  observationsLabel,
  resolveAudioDotStatus,
  resolveDotStatus,
} from './advanced.logic'
import type { BenchmarkComparison } from '../../shared/types'
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

describe('observationsLabel', () => {
  it('formatea el conteo de observaciones locales', () => {
    expect(observationsLabel(0)).toContain('0')
    expect(observationsLabel(3)).toContain('3')
  })
})

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

describe('compatibilityLine', () => {
  it('marca validated en verde', () => {
    const line = compatibilityLine({
      assessment: {
        kind: 'validated',
        evidenceId: 'gepard-26.8.26.1-proton-cachyos-11',
      },
      gepardFileVersion: '26.8.26.1',
    })
    expect(line.dot).toBe('ok')
    expect(line.label).toContain('Validated')
  })

  it('marca unknown con recomendación', () => {
    const line = compatibilityLine({
      assessment: { kind: 'unknown' },
      recommendation: {
        profile: 'managedProtonCachyos11',
        evidenceId: 'gepard-26.8.26.1-proton-cachyos-11',
        reason: 'validatedGepardHash',
      },
    })
    expect(line.dot).toBe('warning')
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

  it('marca compatibilidad unknown como problema', () => {
    expect(
      advancedHasIssue({
        ...healthyStatus,
        compatibility: {
          assessment: { kind: 'unknown' },
          recommendation: {
            profile: 'managedProtonCachyos11',
            evidenceId: 'gepard-26.8.26.1-proton-cachyos-11',
            reason: 'validatedGepardHash',
          },
        },
      }),
    ).toBe(true)
  })
})

describe('benchmarksLabel', () => {
  it('incluye el conteo', () => {
    expect(benchmarksLabel(3)).toBe('Benchmarks A/B · 3')
  })
})

describe('comparisonSummary', () => {
  const base: BenchmarkComparison = {
    schemaVersion: 1,
    comparability: { kind: 'comparable' },
    left: {
      arm: 'left',
      attempts: 1,
      captureSuccesses: 1,
      planIds: ['p1'],
      frametime: {
        p50Ms: 10,
        p95Ms: 12,
        p99Ms: 14,
        onePercentLowFps: 80,
        pointOnePercentLowFps: 70,
        sampleCount: 100,
        runCount: 1,
      },
    },
    right: {
      arm: 'right',
      attempts: 1,
      captureSuccesses: 1,
      planIds: ['p1'],
      frametime: {
        p50Ms: 11,
        p95Ms: 13,
        p99Ms: 15,
        onePercentLowFps: 75,
        pointOnePercentLowFps: 65,
        sampleCount: 100,
        runCount: 1,
      },
    },
    deltas: null,
    gates: {
      visualCorrectLeft: true,
      visualCorrectRight: true,
      startupCleanLeft: true,
      startupCleanRight: true,
      frametimeAvailable: true,
      passed: true,
    },
    caveats: [],
    orderBiasUncontrolled: true,
    usableForAutotune: true,
  }

  it('no declara ganador cuando los gates pasan', () => {
    const s = comparisonSummary(base)
    expect(s.label).toBe('Comparable')
    expect(s.label.toLowerCase()).not.toContain('gan')
    expect(s.label.toLowerCase()).not.toContain('winner')
  })

  it('reporta incomparable', () => {
    const s = comparisonSummary({
      ...base,
      comparability: { kind: 'incomparable', reason: 'scene-mismatch' },
    })
    expect(s.label).toBe('Incomparable')
    expect(s.hint).toBe('scene-mismatch')
  })

  it('no declara ganador con p50 distinto y visual incorrecto', () => {
    const s = comparisonSummary({
      ...base,
      gates: { ...base.gates, visualCorrectLeft: false, passed: false },
      left: {
        ...base.left,
        frametime: { ...base.left.frametime!, p50Ms: 5 },
      },
      right: {
        ...base.right,
        frametime: { ...base.right.frametime!, p50Ms: 50 },
      },
    })
    expect(s.label).toBe('Gates no superados')
    expect(s.label.toLowerCase()).not.toContain('gan')
  })
})
