import type {
  CreateInstallation,
  InstallationDraft,
  InstallationStatus,
  RunnerInfo,
} from '../../shared/types'

export function installationIsActive(status: InstallationStatus) {
  return ['preparing', 'running', 'stopping'].includes(status)
}

export function installationCanRestart(status: InstallationStatus) {
  return ['pending', 'failed', 'interrupted', 'awaitingClient'].includes(status)
}

export function installationExecutableDirectory(draft?: InstallationDraft) {
  return draft?.binding
    ? `${draft.binding.prefixPath}/drive_c/`
    : (draft?.destinationPath ?? undefined)
}

export const INSTALLATION_LABELS: Record<InstallationStatus, string> = {
  pending: 'Lista para instalar',
  preparing: 'Preparando el entorno',
  running: 'Instalador abierto',
  stopping: 'Deteniendo la instalación',
  awaitingClient: 'Cliente pendiente de confirmar',
  failed: 'La instalación necesita atención',
  interrupted: 'Instalación interrumpida',
  completed: 'Servidor agregado',
}

export function validateInstallationInput(
  input: CreateInstallation,
  runners: RunnerInfo[],
): string | null {
  if (!input.name.trim() || input.name.trim().length > 80)
    return 'Escribe un nombre de hasta 80 caracteres'
  if (
    !input.installerPath.startsWith('/') ||
    !/\.exe$/i.test(input.installerPath)
  )
    return 'Selecciona un instalador .exe'
  if (
    input.destinationPath != null &&
    (!input.destinationPath.startsWith('/') || input.destinationPath === '/')
  )
    return 'Selecciona una carpeta externa válida'
  if (!runners.some((runner) => runner.path === input.runner))
    return 'Selecciona uno de los runners administrados'
  return null
}

export function initialInstallationRunner(
  runners: RunnerInfo[],
  selected: string,
) {
  return (
    runners.find((runner) => runner.path === selected)?.path ??
    runners[0]?.path ??
    ''
  )
}
