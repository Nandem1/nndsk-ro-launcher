import type { RuntimeCheck } from '../../shared/types'
import type { DotStatus } from '../../shared/ui/StatusDot'
import type { Tone } from '../../shared/ui/types'
import { resolveDotStatus } from './advanced.logic'

// Display only: pending is actionable, not failed. Backend readiness, setup
// availability and error severities remain untouched.
export function diagnosticDotStatus(
  checks: RuntimeCheck[],
  id: string,
  ok: boolean,
  warning?: string | null,
): DotStatus {
  const matching = checks.filter((check) => check.id === id)
  if (matching.some((check) => check.severity === 'error')) return 'error'
  if (matching.some((check) => ['pending', 'warning'].includes(check.severity)))
    return 'warning'
  return resolveDotStatus(ok, warning)
}

export function diagnosticPanelTone(statuses: DotStatus[]): Tone {
  if (statuses.includes('error')) return 'bad'
  if (statuses.includes('warning')) return 'warn'
  if (statuses.includes('ok')) return 'ok'
  return 'neutral'
}
