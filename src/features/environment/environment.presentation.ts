import type {
  EnvironmentAssessment,
  EnvironmentDomain,
  EnvironmentIssue,
} from '../../shared/types'

export const ENVIRONMENT_GROUPS: {
  domain: EnvironmentDomain
  label: string
}[] = [
  { domain: 'system', label: 'Sistema' },
  { domain: 'tools', label: 'Herramientas de preparación' },
  { domain: 'runner', label: 'Runtime' },
  { domain: 'prefix', label: 'Entorno del servidor' },
  { domain: 'graphics', label: 'Gráficos' },
  { domain: 'feature', label: 'Funciones opcionales' },
  { domain: 'compatibility', label: 'Compatibilidad' },
]

export function environmentSummary(assessment: EnvironmentAssessment): string {
  if (assessment.launch === 'blocked') return 'Hay requisitos pendientes'
  if (assessment.launch === 'needsPreparation') return 'Listo para preparar'
  return assessment.preparationAvailable
    ? 'Entorno listo'
    : 'Requisitos para reparar'
}

export function issueScope(issue: EnvironmentIssue): string {
  if (issue.domain === 'compatibility') return 'Recomendación de compatibilidad'
  if (issue.affects.includes('launch')) return 'Para jugar y preparar'
  if (issue.affects.includes('prepare')) return 'Para preparar o rearmar'
  return 'Función opcional'
}

export function preparationFailure(
  assessment: EnvironmentAssessment | null | undefined,
): string | null {
  if (!assessment || assessment.launch !== 'blocked') return null
  const failures = assessment.issues.filter(
    (issue) => issue.severity === 'error',
  )
  if (!failures.length)
    return 'El entorno necesita revisión. Abre el diagnóstico para ver los requisitos.'
  return `${failures.map((issue) => issue.title).join(' · ')}. Abre el diagnóstico para ver cómo resolverlo.`
}
