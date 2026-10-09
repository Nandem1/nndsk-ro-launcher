import type { RunnerInfo, ServerConfig } from './types'

/** Catálogo ofrecido más la selección anterior, sin convertirla en una migración. */
export function runnerSelectionOptions(
  runners: RunnerInfo[],
  savedPath: string,
) {
  const options = runners.map((runner) => ({
    value: runner.path,
    label: runner.name,
  }))
  if (savedPath && !runners.some((runner) => runner.path === savedPath)) {
    options.push({
      value: savedPath,
      label: `Selección anterior conservada · ${savedPath}`,
    })
  }
  return options
}

export function resolveRunner(
  server: ServerConfig,
  selectedRunner: string,
): string | null {
  return server.runner?.trim() || selectedRunner || null
}

/** Normaliza campos legacy antes de enviar un servidor al runtime. */
export function withResolvedRunner(
  server: ServerConfig,
  selectedRunner: string,
): ServerConfig {
  return {
    ...server,
    prefixMode: 'isolated',
    winePrefix: null,
    runner: server.runner?.trim() || selectedRunner.trim() || null,
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
    server.runner ?? '',
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
