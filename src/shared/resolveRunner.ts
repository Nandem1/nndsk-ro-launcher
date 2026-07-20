import type { ServerConfig } from './types'

export function resolveRunner(
  _server: ServerConfig,
  selectedRunner: string,
): string | null {
  return selectedRunner || null
}

/** Normaliza campos legacy antes de enviar un servidor al runtime. */
export function withResolvedRunner(
  server: ServerConfig,
  selectedRunner: string,
): ServerConfig {
  void selectedRunner
  return {
    ...server,
    prefixMode: 'isolated',
    winePrefix: null,
    runner: null,
  }
}

/** Identidad de la parte del servidor que cambia runner, prefix o requisitos del entorno. */
export function runtimeConfigKey(server: ServerConfig): string {
  return JSON.stringify([
    server.id,
    server.name,
    server.executablePath,
    server.patcherPath ?? '',
    server.launch?.strategy ?? 'direct',
    server.launch?.requireWebview2 ?? false,
  ])
}

/** Identidad del diagnóstico para impedir mezclar resultados de otro runner/servidor. */
export function runtimeStatusKey(
  server: ServerConfig | null,
  selectedRunner: string,
): string {
  return JSON.stringify([
    server ? runtimeConfigKey(server) : null,
    server ? resolveRunner(server, selectedRunner) : selectedRunner || null,
  ])
}

/** Snapshot completo de lo que puede cambiar un lanzamiento en curso. */
export function launchConfigKey(
  server: ServerConfig,
  selectedRunner: string,
): string {
  const strategy = server.launch?.strategy ?? 'direct'
  const activeArgs =
    strategy === 'patcher'
      ? (server.launch?.patcherArgs ?? [])
      : (server.launch?.gameArgs ?? [])
  return JSON.stringify([runtimeStatusKey(server, selectedRunner), activeArgs])
}
