import type {
  AppSettings,
  LaunchConfig,
  LaunchStrategy,
  LaunchValues,
  ServerConfig,
} from './types'

export const SERVER_CONTRACT = {
  maxIdLength: 128,
  maxNameLength: 80,
  executableExtension: '.exe',
  maxLaunchArguments: 64,
  maxLaunchArgumentLength: 4_096,
  maxLaunchTotalLength: 16_384,
  maxLaunchFields: 16,
  maxLaunchFieldKeyLength: 32,
  maxLaunchValueLength: 4_096,
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
  if (
    typeof server.winePrefix === 'string' &&
    (!server.winePrefix.trim().startsWith('/') ||
      server.winePrefix.trim() === '/')
  ) {
    return 'El WINEPREFIX personalizado debe ser una ruta absoluta y no puede ser /'
  }
  if (
    server.prefixMode !== undefined &&
    server.prefixMode !== null &&
    !['shared', 'isolated', 'custom'].includes(server.prefixMode)
  ) {
    return 'El modo de WINEPREFIX no es válido'
  }
  if (server.prefixMode === 'custom' && server.winePrefix == null) {
    return 'El modo de prefijo custom requiere un WINEPREFIX'
  }
  if (
    (server.prefixMode === 'shared' || server.prefixMode === 'isolated') &&
    server.winePrefix != null
  ) {
    return 'winePrefix sólo puede definirse cuando prefixMode es custom o legacy'
  }
  if (typeof server.runner === 'string' && !server.runner.trim()) {
    return 'El runner no puede estar vacío'
  }
  if (server.prefixMode === 'shared' && server.runner) {
    return 'Un runner por servidor requiere un prefijo aislado o custom'
  }
  if (server.launch !== undefined) {
    if (!server.launch || typeof server.launch !== 'object') {
      return 'La configuración de lanzamiento no es válida'
    }
    const launchError = validateLaunchConfig(server.launch)
    if (launchError) return launchError
    if (
      effectiveLaunchStrategy(server.launch) === 'patcher' &&
      !server.patcherPath
    ) {
      return 'El inicio mediante patcher requiere configurar patcherPath'
    }
  }
  return null
}

export function validateLaunchConfig(launch: LaunchConfig): string | null {
  const unknownKeys = Object.keys(launch).filter(
    (key) =>
      !['strategy', 'gameArgs', 'patcherArgs', 'requireWebview2'].includes(key),
  )
  if (unknownKeys.length) {
    return `La configuración de lanzamiento contiene campos desconocidos: ${unknownKeys.join(', ')}`
  }

  const strategy = effectiveLaunchStrategy(launch)
  if (strategy !== 'direct' && strategy !== 'patcher') {
    return 'La estrategia de lanzamiento no es válida'
  }
  if (
    launch.requireWebview2 !== undefined &&
    typeof launch.requireWebview2 !== 'boolean'
  ) {
    return 'El override de WebView2 no es válido'
  }

  const gameError = validateArgumentList('del juego', launch.gameArgs)
  if (gameError) return gameError
  return validateArgumentList('del patcher', launch.patcherArgs)
}

export function requiredLaunchFields(launch?: LaunchConfig): string[] {
  if (!launch) return []
  const args =
    effectiveLaunchStrategy(launch) === 'patcher'
      ? (launch.patcherArgs ?? [])
      : (launch.gameArgs ?? [])
  const result = collectLaunchFields(args)
  if (result.error) throw new Error(result.error)
  return result.fields
}

export function extractLaunchFields(args: string[]): string[] {
  const result = collectLaunchFields(args)
  if (result.error) throw new Error(result.error)
  return result.fields
}

export function validateLaunchValues(
  launch: LaunchConfig | undefined,
  values: LaunchValues,
): string | null {
  let required: string[]
  try {
    required = requiredLaunchFields(launch)
  } catch (error) {
    return error instanceof Error
      ? error.message
      : 'Los campos de lanzamiento no son válidos'
  }

  const entries = Object.entries(values)
  if (entries.length > SERVER_CONTRACT.maxLaunchFields) {
    return `No se pueden proporcionar más de ${SERVER_CONTRACT.maxLaunchFields} valores de lanzamiento`
  }

  const expected = new Set(required)
  for (const [key, value] of entries) {
    const keyError = validateLaunchFieldKey(key)
    if (keyError) return keyError
    if (!expected.has(key)) {
      return `El campo de lanzamiento '\${${key}}' no es esperado`
    }
    if (typeof value !== 'string') {
      return `El valor del campo de lanzamiento '\${${key}}' no es válido`
    }
    if (!value.length) {
      return `El valor del campo de lanzamiento '\${${key}}' no puede estar vacío`
    }
    if (containsControlCharacter(value)) {
      return `El valor del campo de lanzamiento '\${${key}}' contiene un carácter de control`
    }
    if (codePointLength(value) > SERVER_CONTRACT.maxLaunchValueLength) {
      return `El valor del campo de lanzamiento '\${${key}}' supera ${SERVER_CONTRACT.maxLaunchValueLength} caracteres`
    }
  }

  for (const key of required) {
    if (!Object.prototype.hasOwnProperty.call(values, key)) {
      return `Falta el valor del campo de lanzamiento '\${${key}}'`
    }
  }
  return null
}

export function isSensitiveLaunchKey(key: string): boolean {
  const normalized = key.toLowerCase()
  return (
    normalized.includes('password') ||
    normalized.includes('passwd') ||
    normalized.includes('token') ||
    normalized.includes('secret') ||
    normalized.includes('credential') ||
    normalized.includes('clave') ||
    normalized === 'pwd' ||
    normalized === 'pin' ||
    normalized === 'pass' ||
    normalized.endsWith('_pass') ||
    normalized.endsWith('-pass')
  )
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

function effectiveLaunchStrategy(launch: LaunchConfig): LaunchStrategy {
  return launch.strategy ?? 'direct'
}

function validateArgumentList(
  label: string,
  args: string[] | undefined,
): string | null {
  if (args === undefined) return null
  if (!Array.isArray(args)) return `Los argumentos ${label} no son válidos`
  if (args.length > SERVER_CONTRACT.maxLaunchArguments) {
    return `Los argumentos ${label} no pueden superar ${SERVER_CONTRACT.maxLaunchArguments} elementos`
  }

  let totalLength = 0
  for (const [index, argument] of args.entries()) {
    if (typeof argument !== 'string') {
      return `El argumento ${label} ${index + 1} no es válido`
    }
    if (!argument.trim()) {
      return `El argumento ${label} ${index + 1} no puede estar vacío`
    }
    if (containsControlCharacter(argument)) {
      return `El argumento ${label} ${index + 1} contiene un carácter de control`
    }
    const length = codePointLength(argument)
    if (length > SERVER_CONTRACT.maxLaunchArgumentLength) {
      return `El argumento ${label} ${index + 1} no puede superar ${SERVER_CONTRACT.maxLaunchArgumentLength} caracteres`
    }
    totalLength += length
    if (totalLength > SERVER_CONTRACT.maxLaunchTotalLength) {
      return `Los argumentos ${label} no pueden superar ${SERVER_CONTRACT.maxLaunchTotalLength} caracteres en total`
    }
  }

  const fields = collectLaunchFields(args)
  return fields.error
}

function collectLaunchFields(args: string[]): {
  fields: string[]
  error: string | null
} {
  const fields: string[] = []
  const seen = new Set<string>()

  for (const [index, argument] of args.entries()) {
    let cursor = 0
    while (true) {
      const start = argument.indexOf('${', cursor)
      if (start === -1) break
      const end = argument.indexOf('}', start + 2)
      if (end === -1) {
        return {
          fields: [],
          error: `El argumento ${index + 1} contiene un campo de lanzamiento sin cerrar`,
        }
      }
      const key = argument.slice(start + 2, end)
      const keyError = validateLaunchFieldKey(key)
      if (keyError) return { fields: [], error: keyError }
      if (!seen.has(key)) {
        if (fields.length >= SERVER_CONTRACT.maxLaunchFields) {
          return {
            fields: [],
            error: `Los argumentos no pueden usar más de ${SERVER_CONTRACT.maxLaunchFields} campos`,
          }
        }
        seen.add(key)
        fields.push(key)
      }
      cursor = end + 1
    }
  }
  return { fields, error: null }
}

function validateLaunchFieldKey(key: string): string | null {
  const valid = /^[A-Za-z][A-Za-z0-9_-]*$/.test(key)
  if (!valid || key.length > SERVER_CONTRACT.maxLaunchFieldKeyLength) {
    return `La clave de lanzamiento '\${${key}}' no es válida; usa una letra inicial y hasta ${SERVER_CONTRACT.maxLaunchFieldKeyLength} letras, números, '_' o '-'`
  }
  return null
}

function codePointLength(value: string): number {
  return Array.from(value).length
}

function containsControlCharacter(value: string): boolean {
  return Array.from(value).some((character) => {
    const code = character.codePointAt(0) ?? 0
    return code <= 0x1f || (code >= 0x7f && code <= 0x9f)
  })
}
