import { useCallback, useEffect, useState } from 'react'
import { api } from '../../shared/api'
import { runSafely } from '../../shared/async'
import { withResolvedRunner } from '../../shared/resolveRunner'
import type { AutopotConfig, AutopotStatusEvent, ServerConfig } from '../../shared/types'
import { useLauncherStore } from '../launcher/launcher.store'
import { useLogsStore } from '../logs/logs.store'
import { useSettingsStore } from '../settings/settings.store'
import { useServersStore } from '../servers/servers.store'
import {
  mergeAutopotConfig,
  type PersistedAutopotPatch,
  withAutopotPatch,
} from './autopot.logic'
import { useAutopotStore } from './autopot.store'
import { LAUNCHER_EVENTS } from '../../shared/constants'
import { useTauriEvent } from '../../shared/hooks/useTauriEvent'

export function useAutopot(server: ServerConfig | null) {
  const launcherStatus = useLauncherStore((s) => s.status)
  const selectedRunner = useSettingsStore((s) => s.selectedRunner)
  const updateServer = useServersStore((s) => s.updateServer)
  const addToolLog = useLogsStore((s) => s.addToolLog)
  const status = useAutopotStore((s) => s.status)
  const busy = useAutopotStore((s) => s.busy)
  const userEnabled = useAutopotStore((s) => s.userEnabled)
  const setStatus = useAutopotStore((s) => s.setStatus)
  const setBusy = useAutopotStore((s) => s.setBusy)
  const setUserEnabled = useAutopotStore((s) => s.setUserEnabled)
  const reset = useAutopotStore((s) => s.reset)
  const [startError, setStartError] = useState<string | null>(null)

  const persistedConfig = mergeAutopotConfig(server?.autopot)
  const config: AutopotConfig = { ...persistedConfig, enabled: userEnabled }
  const isRunning = launcherStatus === 'running'

  useTauriEvent<AutopotStatusEvent>(
    LAUNCHER_EVENTS.AUTOPOT_STATUS,
    (payload) => setStatus(payload),
    [setStatus],
  )

  useEffect(() => {
    if (!isRunning) {
      reset()
      setStartError(null)
    }
  }, [isRunning, reset])

  const persistConfig = useCallback(
    async (patch: PersistedAutopotPatch): Promise<AutopotConfig | null> => {
      if (!server) return null
      const nextAutopot = withAutopotPatch(mergeAutopotConfig(server.autopot), patch)
      await updateServer(server.id, { autopot: nextAutopot })
      return nextAutopot
    },
    [server, updateServer],
  )

  const startAutopotSafely = useCallback(
    async (autopotConfig: AutopotConfig, failLabel: string) => {
      if (!server) return false
      const resolved = withResolvedRunner({ ...server, autopot: autopotConfig }, selectedRunner)
      const result = await runSafely(() => api.startAutopot(resolved))
      if (!result.ok) {
        setStartError(result.error)
        addToolLog(`[AutoPot] ${failLabel}: ${result.error}`)
      }
      return result.ok
    },
    [addToolLog, selectedRunner, server],
  )

  const setEnabled = useCallback(
    async (enabled: boolean) => {
      if (!server || !isRunning) return
      setBusy(true)
      setStartError(null)
      setUserEnabled(enabled)
      try {
        if (enabled) {
          addToolLog('[AutoPot] Solicitando inicio...')
          const ok = await startAutopotSafely(
            mergeAutopotConfig(server.autopot),
            'Start falló',
          )
          if (!ok) setUserEnabled(false)
        } else {
          await api.stopAutopot()
          addToolLog('[AutoPot] Detenido por usuario')
        }
      } finally {
        setBusy(false)
      }
    },
    [
      addToolLog,
      isRunning,
      server,
      setBusy,
      setUserEnabled,
      startAutopotSafely,
    ],
  )

  const updateField = useCallback(
    async (patch: PersistedAutopotPatch) => {
      if (!server) return
      const nextAutopot = await persistConfig(patch)
      if (!nextAutopot || !useAutopotStore.getState().status.active) return

      const result = await runSafely(() => api.updateAutopotConfig(nextAutopot))
      if (!result.ok) {
        setStartError(result.error)
        addToolLog(`[AutoPot] Config falló: ${result.error}`)
      }
    },
    [addToolLog, persistConfig, server],
  )

  const error = startError ?? status.error

  return {
    config,
    status,
    busy,
    isRunning,
    error,
    setEnabled,
    updateField,
  }
}
