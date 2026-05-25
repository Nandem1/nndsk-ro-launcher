import { useLaunchGame } from './useLaunchGame'
import { useSelectedServer } from '../servers/useSelectedServer'
import { useSettingsStore } from '../settings/settings.store'

export function LaunchButton() {
  const server = useSelectedServer()
  const prefixConfigured = useSettingsStore((s) => s.prefixConfigured)
  const { status, setupProgress, error, isBusy, handleLaunch, handleStop } = useLaunchGame(server)

  const isDisabled = !server || isBusy
  const buildMode = status === 'idle' && !prefixConfigured

  const labels: Record<typeof status, string> = {
    idle: buildMode ? 'CONSTRUIR ENTORNO' : 'JUGAR',
    'setting-up': setupProgress?.step ?? 'Configurando...',
    launching: 'Iniciando...',
    running: 'Jugando...',
    error: 'Reintentar',
  }

  const baseBtn = `w-full py-3.5 px-6 rounded-xl font-bold text-lg tracking-[0.2em] transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed disabled:shadow-none`
  const buttonClass = buildMode
    ? `${baseBtn} bg-gradient-to-b from-zinc-500 to-zinc-600 hover:from-zinc-400 hover:to-zinc-500 active:from-zinc-600 active:to-zinc-700 text-white shadow-lg shadow-zinc-500/10 disabled:hover:from-zinc-500 disabled:hover:to-zinc-600`
    : `${baseBtn} bg-gradient-to-b from-amber-400 to-amber-500 hover:from-amber-300 hover:to-amber-400 active:from-amber-500 active:to-amber-600 text-zinc-950 shadow-lg shadow-amber-500/10 disabled:hover:from-amber-400 disabled:hover:to-amber-500`

  return (
    <div className="flex flex-col gap-2 shrink-0">
      {status === 'setting-up' && setupProgress && (
        <div className="w-full bg-zinc-800/80 rounded-full h-1 overflow-hidden">
          <div
            className="bg-gradient-to-r from-amber-600 to-amber-400 h-1 rounded-full transition-all duration-500"
            style={{ width: `${setupProgress.percent}%` }}
          />
        </div>
      )}
      <button
        onClick={handleLaunch}
        disabled={isDisabled}
        className={buttonClass}
      >
        {labels[status]}
      </button>
      {status === 'running' && (
        <button
          onClick={handleStop}
          className="w-full py-2 rounded-xl text-xs text-zinc-500 hover:text-red-400 border border-zinc-800/80
            hover:border-red-500/30 hover:bg-red-500/5 transition-colors"
        >
          Detener juego
        </button>
      )}
      {status === 'error' && error && (
        <p className="text-red-400 text-xs text-center px-2 leading-relaxed">{error}</p>
      )}
    </div>
  )
}
