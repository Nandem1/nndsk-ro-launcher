import type { EnvironmentAssessment } from '../../shared/types'

export const missingTools: EnvironmentAssessment = {
  schemaVersion: 1,
  provider: 'wineHostTools',
  launch: 'blocked',
  preparationAvailable: false,
  rebuildAvailable: false,
  issues: ['cabextract', 'unzip'].map((tool) => ({
    code: `host-${tool}`,
    domain: 'tools',
    severity: 'error',
    affects: ['prepare', 'rebuild'],
    title: `Herramienta ${tool}`,
    message: `Falta ${tool}`,
    remediation: `Instala ${tool} con el administrador de paquetes`,
  })),
  actions: [
    { kind: 'recheck' },
    { kind: 'hostGuide', family: 'arch', packages: ['cabextract', 'unzip'] },
  ],
}
