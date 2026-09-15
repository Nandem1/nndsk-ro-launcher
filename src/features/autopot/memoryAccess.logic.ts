export type MemoryAccess =
  | 'processVmReadv'
  | 'procMem'
  | 'noReadableWritableRegion'
  | 'outsideSupervisor'
  | 'yamaDenied'
  | 'processExitedOrReused'
  | 'backendError'

export type ProfileMemory =
  'notConfigured' | 'addressUnmapped' | 'invalidRead' | 'valid'

export function memoryAccessUsable(
  access: MemoryAccess | null | undefined,
): boolean {
  return access === 'processVmReadv' || access === 'procMem'
}

export function memoryAccessLabel(access: MemoryAccess): string {
  switch (access) {
    case 'processVmReadv':
      return 'Disponible mediante process_vm_readv'
    case 'procMem':
      return 'Disponible mediante /proc/<pid>/mem'
    case 'noReadableWritableRegion':
      return 'Sin región legible/escribible para preflight'
    case 'outsideSupervisor':
      return 'Proceso fuera del supervisor'
    case 'yamaDenied':
      return 'Permiso denegado por Yama'
    case 'processExitedOrReused':
      return 'Proceso terminado o PID reutilizado'
    case 'backendError':
      return 'Backend de memoria no disponible'
  }
}

export function profileMemoryLabel(profile: ProfileMemory): string {
  switch (profile) {
    case 'notConfigured':
      return 'No configurado'
    case 'addressUnmapped':
      return 'Dirección no mapeada'
    case 'invalidRead':
      return 'Lectura inválida'
    case 'valid':
      return 'Válido'
  }
}

export const MEMORY_RELAY_ACTION =
  'Cierra las instancias antiguas y relanza el juego desde RO-Launcher.'

export function memoryAccessAction(
  access: MemoryAccess | null | undefined,
): string | null {
  if (access === 'outsideSupervisor' || access === 'yamaDenied') {
    return MEMORY_RELAY_ACTION
  }
  return null
}
