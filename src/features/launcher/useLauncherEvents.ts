import { useEffect } from 'react'
import type {
  ExitEventPayload,
  GameClientSnapshot,
  LogEventPayload,
  ProgressPayload,
} from '../../shared/types'
import { isLauncherBusy, useLauncherStore } from './launcher.store'
import { useLogsStore } from '../logs/logs.store'
import { LAUNCHER_EVENTS } from '../../shared/constants'
import { useTauriEvent } from '../../shared/hooks/useTauriEvent'
import { refreshGameClients } from './refreshGameClients'

export function useLauncherEvents() {
  const setStatus = useLauncherStore((s) => s.setStatus)
  const setProgress = useLauncherStore((s) => s.setProgress)
  const setError = useLauncherStore((s) => s.setError)
  const removeClient = useLauncherStore((s) => s.removeClient)
  const upsertClient = useLauncherStore((s) => s.upsertClient)
  const addGameLog = useLogsStore((s) => s.addGameLog)
  const addToolLog = useLogsStore((s) => s.addToolLog)

  useTauriEvent<LogEventPayload>(LAUNCHER_EVENTS.LOG, (payload) =>
    addGameLog(payload.line),
  )

  useTauriEvent<LogEventPayload>(LAUNCHER_EVENTS.TOOL_LOG, (payload) =>
    addToolLog(payload.line),
  )

  useTauriEvent<ProgressPayload>(LAUNCHER_EVENTS.PROGRESS, (payload) => {
    const current = useLauncherStore.getState()
    if (
      current.status === 'setting-up' &&
      current.operationId &&
      payload.operationId === current.operationId
    )
      setProgress(payload)
  })

  const clientsReady = useTauriEvent<GameClientSnapshot>(
    LAUNCHER_EVENTS.GAME_CLIENT,
    (payload) => {
      upsertClient(payload)
    },
  )

  const exitsReady = useTauriEvent<ExitEventPayload>(
    LAUNCHER_EVENTS.GAME_EXIT,
    (payload) => {
      const { clientId, code, requested, serverName } = payload
      removeClient(clientId)
      if (!requested && code !== 0) {
        const msg = `${serverName} cerró inesperadamente (código ${code})`
        addGameLog(msg)
        if (!isLauncherBusy(useLauncherStore.getState().status)) {
          setError(msg)
          setStatus('error')
        }
      } else {
        addGameLog(`${serverName} cerrado`)
      }
    },
  )

  useEffect(() => {
    // Listen before taking the census so an exit between snapshot and response cannot be lost.
    if (!clientsReady || !exitsReady) return
    void refreshGameClients().catch(() =>
      addGameLog('No se pudo sincronizar la lista de clientes'),
    )
  }, [addGameLog, clientsReady, exitsReady])
}
