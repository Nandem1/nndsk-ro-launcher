import type { LaunchConfig, PrefixMode, ServerConfig } from '../../shared/types'
import { SERVER_CONTRACT, validateLaunchConfig } from '../../shared/contracts'

export interface ServerConfigDraft {
  name: string
  executablePath: string
  patcherPath: string
  prefixMode: PrefixMode
  winePrefix: string
  runner: string
  strategy: LaunchConfig['strategy']
  gameArgs: string
  patcherArgs: string
  requireWebview2: boolean
}

export type ServerConfigFields = Pick<
  ServerConfig,
  | 'name'
  | 'executablePath'
  | 'patcherPath'
  | 'prefixMode'
  | 'winePrefix'
  | 'runner'
  | 'launch'
>

const EXE_EXTENSION = /\.exe$/i

export function effectivePrefixMode(server?: ServerConfig): PrefixMode {
  void server
  return 'isolated'
}

export function createServerConfigDraft(
  server?: ServerConfig,
): ServerConfigDraft {
  return {
    name: server?.name ?? '',
    executablePath: server?.executablePath ?? '',
    patcherPath: server?.patcherPath ?? '',
    prefixMode: effectivePrefixMode(server),
    winePrefix: '',
    runner: '',
    strategy: server?.launch?.strategy ?? 'direct',
    gameArgs: argsToText(server?.launch?.gameArgs),
    patcherArgs: argsToText(server?.launch?.patcherArgs),
    requireWebview2: server?.launch?.requireWebview2 ?? false,
  }
}

export function argsToText(args?: string[]): string {
  return args?.join('\n') ?? ''
}

/** Cada línea no vacía representa exactamente un argv; no se tokeniza por espacios. */
export function textToArgs(value: string): string[] {
  return value
    .replace(/\r\n?/g, '\n')
    .split('\n')
    .filter((line) => line.trim().length > 0)
}

export function validateServerConfigDraft(
  draft: ServerConfigDraft,
): string | null {
  if (!draft.name.trim()) return 'Escribe un nombre para el servidor'
  if (draft.name.trim().length > SERVER_CONTRACT.maxNameLength) {
    return `El nombre no puede superar ${SERVER_CONTRACT.maxNameLength} caracteres`
  }
  if (!EXE_EXTENSION.test(draft.executablePath.trim())) {
    return 'Selecciona un ejecutable del juego con extensión .exe'
  }
  if (
    draft.patcherPath.trim() &&
    !EXE_EXTENSION.test(draft.patcherPath.trim())
  ) {
    return 'El patcher debe ser un archivo .exe'
  }
  if (draft.strategy === 'patcher' && !draft.patcherPath.trim()) {
    return 'Selecciona un patcher para usar la estrategia Patcher'
  }
  return validateLaunchConfig({
    strategy: draft.strategy,
    gameArgs: textToArgs(draft.gameArgs),
    patcherArgs: textToArgs(draft.patcherArgs),
    requireWebview2: draft.requireWebview2,
  })
}

export function serverFieldsFromDraft(
  draft: ServerConfigDraft,
): ServerConfigFields {
  return {
    name: draft.name.trim(),
    executablePath: draft.executablePath.trim(),
    patcherPath: draft.patcherPath.trim() || null,
    prefixMode: 'isolated',
    winePrefix: null,
    runner: null,
    launch: {
      strategy: draft.strategy,
      gameArgs: textToArgs(draft.gameArgs),
      patcherArgs: textToArgs(draft.patcherArgs),
      requireWebview2: draft.requireWebview2,
    },
  }
}
