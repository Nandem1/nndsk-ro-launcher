import { runSafely } from '../../shared/async'
import { useLogsStore } from '../logs/logs.store'
import { useLauncherStore, isLauncherBusy } from './launcher.store'

// Ownership survives a component unmount (including a rail/layout change).
let taskInFlight = false

/** Ejecuta una tarea async y sincroniza error/estado con el store del launcher. */
export function useLauncherTask() {
  const { status, setupProgress, error, setStatus, setProgress, setError } =
    useLauncherStore()
  const addGameLog = useLogsStore((s) => s.addGameLog)

  const runTask = async <T>(fn: () => Promise<T>, errorPrefix?: string) => {
    if (taskInFlight || isLauncherBusy(useLauncherStore.getState().status)) {
      return { ok: false as const, error: 'Ya hay una operación en curso' }
    }
    taskInFlight = true
    useLauncherStore.setState({ operationId: crypto.randomUUID() })
    try {
      setError(null)
      setStatus('checking')
      const result = await runSafely(fn)
      if (!result.ok) {
        setError(result.error)
        setStatus('error')
        setProgress(null)
        addGameLog(
          errorPrefix
            ? `${errorPrefix}: ${result.error}`
            : `Error: ${result.error}`,
        )
      } else {
        setStatus('idle')
      }
      return result
    } finally {
      taskInFlight = false
      useLauncherStore.setState({ operationId: null })
    }
  }

  return {
    status,
    setupProgress,
    error,
    setStatus,
    setProgress,
    setError,
    addGameLog,
    runTask,
    isBusy: isLauncherBusy(status),
  }
}
