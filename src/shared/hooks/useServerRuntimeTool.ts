import { useCallback, useEffect, useMemo, useState } from 'react'
import { runSafely } from '../async'
import { withResolvedRunner } from '../resolveRunner'
import type { ServerConfig } from '../types'
import { useTauriEvent } from './useTauriEvent'

type RuntimeToolConfig = {
  enabled: boolean
}

interface ToolStateAdapter<Status> {
  status: Status
  busy: boolean
  userEnabled: boolean
  setStatus: (status: Status) => void
  setBusy: (busy: boolean) => void
  setUserEnabled: (enabled: boolean) => void
  reset: () => void
}

interface ToolPersistenceAdapter<Config, Patch> {
  persistedConfig?: Config
  mergeConfig: (config?: Config) => Config
  withPatch: (config: Config, patch: Patch) => Config
  readServerConfig: (server: ServerConfig) => Config | undefined
  persistServer: (
    serverId: string,
    update: (server: ServerConfig) => ServerConfig,
  ) => Promise<ServerConfig | null>
  buildServerConfig: (server: ServerConfig, config: Config) => ServerConfig
}

interface ToolRuntimeAdapter<Config, Status> {
  eventName: string
  toolName: string
  addToolLog: (line: string) => void
  start: (server: ServerConfig) => Promise<void>
  stop: () => Promise<void>
  updateConfig: (config: Config) => Promise<void>
  isActive: () => boolean
  statusError: (status: Status) => string | null | undefined
}

interface ServerRuntimeToolOptions<
  Config extends RuntimeToolConfig,
  Status,
  Patch,
> {
  server: ServerConfig | null
  isRunning: boolean
  selectedRunner: string
  state: ToolStateAdapter<Status>
  persistence: ToolPersistenceAdapter<Config, Patch>
  runtime: ToolRuntimeAdapter<Config, Status>
}

export function useServerRuntimeTool<
  Config extends RuntimeToolConfig,
  Status,
  Patch,
>({
  server,
  isRunning,
  selectedRunner,
  state: {
    status,
    busy,
    userEnabled,
    setStatus,
    setBusy,
    setUserEnabled,
    reset,
  },
  persistence: {
    persistedConfig,
    mergeConfig,
    withPatch,
    readServerConfig,
    persistServer,
    buildServerConfig,
  },
  runtime: {
    eventName,
    toolName,
    addToolLog,
    start: startTool,
    stop: stopTool,
    updateConfig: updateToolConfig,
    isActive: isRuntimeActive,
    statusError,
  },
}: ServerRuntimeToolOptions<Config, Status, Patch>) {
  const [startError, setStartError] = useState<string | null>(null)

  const config = useMemo<Config>(
    () => ({ ...mergeConfig(persistedConfig), enabled: userEnabled }),
    [mergeConfig, persistedConfig, userEnabled],
  )

  useTauriEvent<Status>(eventName, (payload) => setStatus(payload))

  useEffect(() => {
    if (!isRunning) {
      reset()
      setStartError(null)
    }
  }, [isRunning, reset])

  const saveConfig = useCallback(
    async (patch: Patch): Promise<Config | null> => {
      if (!server) return null
      let nextConfig: Config | null = null
      const updated = await persistServer(server.id, (currentServer) => {
        nextConfig = withPatch(
          mergeConfig(readServerConfig(currentServer)),
          patch,
        )
        return buildServerConfig(currentServer, nextConfig)
      })
      return updated ? nextConfig : null
    },
    [
      buildServerConfig,
      mergeConfig,
      persistServer,
      readServerConfig,
      server,
      withPatch,
    ],
  )

  const startSafely = useCallback(
    async (runtimeConfig: Config, failLabel: string) => {
      if (!server) return false
      const resolved = withResolvedRunner(
        buildServerConfig(server, runtimeConfig),
        selectedRunner,
      )
      const result = await runSafely(() => startTool(resolved))
      if (!result.ok) {
        setStartError(result.error)
        addToolLog(`[${toolName}] ${failLabel}: ${result.error}`)
      }
      return result.ok
    },
    [
      addToolLog,
      buildServerConfig,
      selectedRunner,
      server,
      startTool,
      toolName,
    ],
  )

  const setEnabled = useCallback(
    async (enabled: boolean) => {
      if (!server || !isRunning) return
      setBusy(true)
      setStartError(null)
      setUserEnabled(enabled)
      try {
        if (enabled) {
          addToolLog(`[${toolName}] Solicitando inicio...`)
          const ok = await startSafely(
            mergeConfig(persistedConfig),
            'Start falló',
          )
          if (!ok) setUserEnabled(false)
        } else {
          await stopTool()
          addToolLog(`[${toolName}] Detenido por usuario`)
        }
      } finally {
        setBusy(false)
      }
    },
    [
      addToolLog,
      isRunning,
      mergeConfig,
      persistedConfig,
      server,
      setBusy,
      setUserEnabled,
      startSafely,
      stopTool,
      toolName,
    ],
  )

  const updateField = useCallback(
    async (patch: Patch) => {
      if (!server) return
      const nextConfig = await saveConfig(patch)
      if (!nextConfig || !isRuntimeActive()) return

      const result = await runSafely(() => updateToolConfig(nextConfig))
      if (!result.ok) {
        setStartError(result.error)
        addToolLog(`[${toolName}] Config falló: ${result.error}`)
      }
    },
    [
      addToolLog,
      isRuntimeActive,
      saveConfig,
      server,
      toolName,
      updateToolConfig,
    ],
  )

  return {
    config,
    status,
    busy,
    isRunning,
    error: startError ?? statusError(status) ?? null,
    setEnabled,
    updateField,
  }
}
