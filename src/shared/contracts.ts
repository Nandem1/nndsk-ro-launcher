import type { AppSettings, ServerConfig } from './types'

export const SERVER_CONTRACT = {
  maxIdLength: 128,
  maxNameLength: 80,
  executableExtension: '.exe',
} as const

/** Validación de frontera antes de persistir o enviar datos al backend. */
export function validateServerConfig(server: ServerConfig): string | null {
  if (!server.id.trim() || server.id.length > SERVER_CONTRACT.maxIdLength) {
    return 'El identificador del servidor no es válido'
  }
  if (
    !server.name.trim() ||
    server.name.length > SERVER_CONTRACT.maxNameLength
  ) {
    return `El nombre debe tener entre 1 y ${SERVER_CONTRACT.maxNameLength} caracteres`
  }
  if (!hasExeExtension(server.executablePath)) {
    return 'El ejecutable del cliente debe ser un archivo .exe'
  }
  if (
    typeof server.patcherPath === 'string' &&
    !hasExeExtension(server.patcherPath)
  ) {
    return 'El patcher debe ser un archivo .exe'
  }
  if (typeof server.winePrefix === 'string' && !server.winePrefix.trim()) {
    return 'El WINEPREFIX no puede estar vacío'
  }
  if (typeof server.runner === 'string' && !server.runner.trim()) {
    return 'El runner no puede estar vacío'
  }
  return null
}

export function validateServers(servers: ServerConfig[]): string | null {
  const ids = new Set<string>()
  for (const server of servers) {
    const error = validateServerConfig(server)
    if (error) return error
    if (ids.has(server.id))
      return `El identificador '${server.id}' está duplicado`
    ids.add(server.id)
  }
  return null
}

export function validateAppSettings(settings: AppSettings): string | null {
  return settings.defaultRunner.trim()
    ? null
    : 'El runner por defecto no puede estar vacío'
}

function hasExeExtension(path: string): boolean {
  return path.trim().toLowerCase().endsWith(SERVER_CONTRACT.executableExtension)
}
