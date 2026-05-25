import { runSafely } from '../../shared/async'
import { useLogsStore } from '../logs/logs.store'
import { useLauncherStore, isLauncherBusy } from './launcher.store'

/** Ejecuta una tarea async y sincroniza error/estado con el store del launcher. */
export function useLauncherTask() {
  const { status, setupProgress, error, setStatus, setProgress, setError } = useLauncherStore()
  const addGameLog = useLogsStore((s) => s.addGameLog)

  const runTask = async (fn: () => Promise<void>, errorPrefix?: string) => {
    const result = await runSafely(fn)
    if (!result.ok) {
      setError(result.error)
      setStatus('error')
      setProgress(null)
      addGameLog(errorPrefix ? `${errorPrefix}: ${result.error}` : `Error: ${result.error}`)
    }
    return result
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
