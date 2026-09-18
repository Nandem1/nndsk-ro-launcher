import { audioFromDeps } from '../../shared/audio'
import type {
  AdvancedDepsStatus,
  CompatibilityStatus,
  DependencyStatus,
} from '../../shared/types'
import type { DotStatus } from '../../shared/ui/StatusDot'

export function resolveDotStatus(
  ok: boolean,
  warning?: string | null,
): DotStatus {
  if (ok && !warning) return 'ok'
  if (!ok) return 'error'
  return 'warning'
}

export function resolveAudioDotStatus(
  ok: boolean,
  warning?: string | null,
): DotStatus {
  if (!ok) return 'error'
  if (warning) return 'warning'
  return 'ok'
}

export function compatibilityLine(compatibility: CompatibilityStatus): {
  dot: DotStatus
  label: string
  hint: string
} {
  if (compatibility.assessment.kind === 'validated') {
    const evidence = compatibility.assessment.evidenceId ?? 'evidence'
    const version = compatibility.gepardFileVersion ?? ''
    return {
      dot: 'ok',
      label: 'Compatibilidad · Validated',
      hint: `${evidence}${version ? ` · ${version}` : ''}`,
    }
  }
  if (compatibility.recommendation) {
    return {
      dot: 'warning',
      label: 'Compatibilidad · Unknown',
      hint: `${compatibility.recommendation.profile} · ${compatibility.recommendation.evidenceId}`,
    }
  }
  const prefix = compatibility.gepardSha256Prefix
  return {
    dot: 'warning',
    label: 'Compatibilidad · Unknown',
    hint: prefix
      ? `Gepard SHA-256 ${prefix}… sin record curated`
      : 'No se pudo leer gepard.dll',
  }
}

export function dxvkHintFromDeps(
  deps: Pick<DependencyStatus, 'dxvkWarning' | 'runtimePlan'>,
): string | null {
  if (deps.dxvkWarning) {
    return deps.dxvkWarning
  }
  const plan = deps.runtimePlan
  if (!plan) {
    return null
  }
  return `${plan.dxvkProvider} · ${plan.dxvkComponentId}`
}

export function advancedStatusFromDeps(
  deps: DependencyStatus,
): AdvancedDepsStatus {
  return {
    ...audioFromDeps(deps),
    inputGroupOk: deps.inputGroupOk,
    inputGroupWarning: deps.inputGroupWarning,
    uinputInputOk: deps.uinputInputOk,
    uinputInputWarning: deps.uinputInputWarning,
    prefixOk: deps.prefixOk,
    prefixWarning: deps.prefixWarning,
    dxvkOk: deps.dxvkOk,
    dxvk: deps.dxvk,
    dxvkWarning: deps.dxvkWarning,
    runnerKind: deps.runnerKind,
    runnerOk: deps.runnerOk,
    runnerWarning: deps.runnerWarning,
    prefixPath: deps.prefixPath,
    prefixScope: deps.prefixScope,
    prefixManaged: deps.prefixManaged,
    readyToLaunch: deps.readyToLaunch,
    canSetup: deps.canSetup,
    canReset: deps.canReset,
    checks: deps.checks,
    runtimePlan: deps.runtimePlan,
    compatibility: deps.compatibility,
  }
}

export function advancedHasIssue(status: AdvancedDepsStatus): boolean {
  const compatibilityIssue =
    status.compatibility != null &&
    status.compatibility.assessment.kind !== 'validated'
  return (
    compatibilityIssue ||
    resolveDotStatus(status.runnerOk, status.runnerWarning) !== 'ok' ||
    resolveAudioDotStatus(status.audioOk, status.audioWarning) !== 'ok' ||
    resolveDotStatus(status.prefixOk, status.prefixWarning) !== 'ok' ||
    resolveDotStatus(status.uinputInputOk, status.uinputInputWarning) !==
      'ok' ||
    resolveDotStatus(status.dxvkOk, status.dxvkWarning) !== 'ok'
  )
}
