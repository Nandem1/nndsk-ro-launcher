import type {
  ExitEventPayload,
  LogEventPayload,
  ProgressPayload,
} from '../../shared/types'
import { useLauncherStore } from './launcher.store'
import { useLogsStore } from '../logs/logs.store'
import { LAUNCHER_EVENTS } from '../../shared/constants'
import { useTauriEvent } from '../../shared/hooks/useTauriEvent'

export function useLauncherEvents() {
  const setStatus = useLauncherStore((s) => s.setStatus)
  const setProgress = useLauncherStore((s) => s.setProgress)
  const setError = useLauncherStore((s) => s.setError)
  const addGameLog = useLogsStore((s) => s.addGameLog)
  const addToolLog = useLogsStore((s) => s.addToolLog)

  useTauriEvent<LogEventPayload>(LAUNCHER_EVENTS.LOG, (payload) =>
    addGameLog(payload.line),
  )

  useTauriEvent<LogEventPayload>(LAUNCHER_EVENTS.TOOL_LOG, (payload) =>
    addToolLog(payload.line),
  )

  useTauriEvent<ProgressPayload>(LAUNCHER_EVENTS.PROGRESS, (payload) =>
    setProgress(payload),
  )

  useTauriEvent<ExitEventPayload>(LAUNCHER_EVENTS.GAME_EXIT, (payload) => {
    const { code } = payload
    if (code !== 0) {
      const msg = `El juego cerró inesperadamente (código ${code})`
      addGameLog(msg)
      setError(msg)
      setStatus('error')
    } else {
      addGameLog('Juego cerrado')
      setStatus('idle')
    }
  })
}
