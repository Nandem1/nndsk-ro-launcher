import type { AutopotInputStatus, DependencyStatus } from './types'

export function autopotInputFromDeps(deps: DependencyStatus): AutopotInputStatus {
  return {
    autopotInputOk: deps.autopotInputOk,
    autopotInputWarning: deps.autopotInputWarning,
  }
}
