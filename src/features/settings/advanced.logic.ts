import { audioFromDeps } from '../../shared/audio'
import type { AdvancedDepsStatus, DependencyStatus } from '../../shared/types'
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
  }
}

export function advancedHasIssue(status: AdvancedDepsStatus): boolean {
  return (
    resolveDotStatus(status.runnerOk, status.runnerWarning) !== 'ok' ||
    resolveAudioDotStatus(status.audioOk, status.audioWarning) !== 'ok' ||
    resolveDotStatus(status.prefixOk, status.prefixWarning) !== 'ok' ||
    resolveDotStatus(status.uinputInputOk, status.uinputInputWarning) !==
      'ok' ||
    resolveDotStatus(status.dxvkOk, status.dxvkWarning) !== 'ok'
  )
}
