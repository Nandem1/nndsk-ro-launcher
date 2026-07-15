import { api } from '../../shared/api'
import { DEFAULT_PREFIX_PATH } from '../../shared/constants'
import { Button } from '../../shared/ui/Button'
import { useLauncherTask } from '../launcher/useLauncherTask'
import { useSettingsStore } from './settings.store'

export function PrefixResetButton() {
  const { setStatus, setProgress, setError, addGameLog, runTask, isBusy } =
    useLauncherTask()
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
    <Button
      variant="secondary"
      size="sm"
      block
      onClick={handleReset}
      disabled={isBusy}
    >
      Rearmar WINEPREFIX
    </Button>
  )
}
