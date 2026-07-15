import { useCallback, useEffect, useMemo, useState } from 'react'
import { runSafely } from '../async'
import { withResolvedRunner } from '../resolveRunner'
import type { ServerConfig } from '../types'
import { useTauriEvent } from './useTauriEvent'

type RuntimeToolConfig = {
  enabled: boolean
}

interface ServerRuntimeToolOptions<
  Config extends RuntimeToolConfig,
  Status,
  Patch,
> {
  server: ServerConfig | null
  isRunning: boolean
  selectedRunner: string
  eventName: string
  toolName: string
  persistedConfig?: Config
  status: Status
  busy: boolean
  userEnabled: boolean
  setStatus: (status: Status) => void
  setBusy: (busy: boolean) => void
  setUserEnabled: (enabled: boolean) => void
  reset: () => void
  addToolLog: (line: string) => void
  mergeConfig: (config?: Config) => Config
  withPatch: (config: Config, patch: Patch) => Config
  persistConfig: (serverId: string, config: Config) => Promise<void>
  startTool: (server: ServerConfig) => Promise<void>
  stopTool: () => Promise<void>
  updateToolConfig: (config: Config) => Promise<void>
  buildServerConfig: (server: ServerConfig, config: Config) => ServerConfig
  isRuntimeActive: () => boolean
  statusError: (status: Status) => string | null | undefined
}

export function useServerRuntimeTool<
  Config extends RuntimeToolConfig,
  Status,
  Patch,
>({
  server,
  isRunning,
  selectedRunner,
  eventName,
  toolName,
  persistedConfig,
  status,
  busy,
  userEnabled,
  setStatus,
  setBusy,
  setUserEnabled,
  reset,
  addToolLog,
  mergeConfig,
  withPatch,
  persistConfig,
  startTool,
  stopTool,
  updateToolConfig,
  buildServerConfig,
  isRuntimeActive,
  statusError,
}: ServerRuntimeToolOptions<Config, Status, Patch>) {
  const [startError, setStartError] = useState<string | null>(null)

  const config = useMemo<Config>(
    () => ({ ...mergeConfig(persistedConfig), enabled: userEnabled }),
    [mergeConfig, persistedConfig, userEnabled],
  )

  useTauriEvent<Status>(eventName, (payload) => setStatus(payload), [setStatus])

  useEffect(() => {
    if (!isRunning) {
      reset()
      setStartError(null)
    }
  }, [isRunning, reset])

  const saveConfig = useCallback(
    async (patch: Patch): Promise<Config | null> => {
      if (!server) return null
      const nextConfig = withPatch(mergeConfig(persistedConfig), patch)
      await persistConfig(server.id, nextConfig)
      return nextConfig
    },
    [mergeConfig, persistConfig, persistedConfig, server, withPatch],
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
