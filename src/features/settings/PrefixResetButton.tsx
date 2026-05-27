import { api } from '../../shared/api'
import { DEFAULT_PREFIX_PATH } from '../../shared/constants'
import { useLauncherTask } from '../launcher/useLauncherTask'
import { useSettingsStore } from './settings.store'

export function PrefixResetButton() {
  const { setStatus, setProgress, setError, addGameLog, runTask, isBusy } = useLauncherTask()
  const selectedRunner = useSettingsStore((s) => s.selectedRunner)
  const loadDepsStatus = useSettingsStore((s) => s.loadDepsStatus)

  const handleReset = async () => {
    const confirmed = window.confirm(
      `¿Rearmar el WINEPREFIX?\n\nSe borrará ${DEFAULT_PREFIX_PATH} y se reinstalarán Gecko, DXVK, vcredist y d3dx9.`,
    )
    if (!confirmed) return

    setError(null)
    setStatus('setting-up')
    addGameLog('Rearmando WINEPREFIX...')

    await runTask(async () => {
      await api.stopGame()
      await api.resetPrefix()
      setProgress(null)
      setStatus('idle')
      addGameLog('WINEPREFIX rearmado correctamente.')
      if (selectedRunner) {
        await loadDepsStatus(selectedRunner)
      }
    }, 'Error al rearmar prefix')
  }

  return (
    <button
      type="button"
      onClick={handleReset}
      disabled={isBusy}
      className="w-full py-1.5 px-3 rounded-lg text-[11px] font-medium text-zinc-400 border border-zinc-800/80
        hover:text-amber-300 hover:border-amber-500/30 hover:bg-amber-500/5 transition-colors
        disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:text-zinc-400 disabled:hover:border-zinc-800/80 disabled:hover:bg-transparent"
    >
      Rearmar WINEPREFIX
    </button>
  )
}
