import { audioFromDeps } from '../../shared/audio'
import { autopotInputFromDeps } from '../../shared/autopotInput'
import type { AdvancedDepsStatus, DependencyStatus } from '../../shared/types'
import type { DotStatus } from '../../shared/ui/StatusDot'

export function resolveDotStatus(ok: boolean, warning?: string | null): DotStatus {
  if (ok && !warning) return 'ok'
  if (!ok) return 'error'
  return 'warning'
}

export function resolveAudioDotStatus(ok: boolean, warning?: string | null): DotStatus {
  if (!ok) return 'error'
  if (warning) return 'warning'
  return 'ok'
}

export function advancedStatusFromDeps(deps: DependencyStatus): AdvancedDepsStatus {
  return {
    ...audioFromDeps(deps),
    ...autopotInputFromDeps(deps),
    inputGroupOk: deps.inputGroupOk,
    inputGroupWarning: deps.inputGroupWarning,
    prefixOk: deps.prefixOk,
    prefixWarning: deps.prefixWarning,
    dxvkOk: deps.dxvkOk,
    dxvkWarning: deps.dxvkWarning,
  }
}

export function advancedHasIssue(status: AdvancedDepsStatus): boolean {
  return (
    resolveAudioDotStatus(status.audioOk, status.audioWarning) !== 'ok' ||
    resolveDotStatus(status.prefixOk, status.prefixWarning) !== 'ok' ||
    resolveDotStatus(status.dxvkOk, status.dxvkWarning) !== 'ok'
  )
}
