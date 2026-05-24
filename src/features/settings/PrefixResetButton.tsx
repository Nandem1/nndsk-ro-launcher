import { invoke } from '@tauri-apps/api/core'
import { useLauncherStore } from '../launcher/launcher.store'
import { useLogsStore } from '../logs/logs.store'

export function PrefixResetButton() {
  const { status, setStatus, setProgress, setError } = useLauncherStore()
  const addLog = useLogsStore((s) => s.addLog)

  const isBusy =
    status === 'setting-up' ||
    status === 'launching' ||
    status === 'running'

  const handleReset = async () => {
    const confirmed = window.confirm(
      '¿Rearmar el WINEPREFIX?\n\nSe borrará ~/.local/share/ro-launcher/prefix y se reinstalarán Gecko, DXVK, vcredist y d3dx9.',
    )
    if (!confirmed) return

    setError(null)
    setStatus('setting-up')
    addLog('Rearmando WINEPREFIX...')

    try {
      await invoke('stop_game')
      await invoke('reset_prefix')
      setProgress(null)
      setStatus('idle')
      addLog('WINEPREFIX rearmado correctamente.')
    } catch (err) {
      const msg = String(err)
      setError(msg)
      setStatus('error')
      setProgress(null)
      addLog(`Error al rearmar prefix: ${msg}`)
    }
  }

  return (
    <button
      type="button"
      onClick={handleReset}
      disabled={isBusy}
      className="w-full py-2 rounded-xl text-xs text-zinc-500 hover:text-amber-400 border border-zinc-800/80
        hover:border-amber-500/30 hover:bg-amber-500/5 transition-colors shrink-0
        disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:text-zinc-500 disabled:hover:border-zinc-800/80 disabled:hover:bg-transparent"
    >
      Rearmar WINEPREFIX
    </button>
  )
}
