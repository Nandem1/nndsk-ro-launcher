import { useLauncherStore } from '../features/launcher/launcher.store'
import { StatusDot } from '../shared/ui/StatusDot'
import { useUiModeStore } from './uiMode.store'

function IngameStatusChip() {
  const launching = useLauncherStore((s) => s.status === 'launching')
  const clients = useLauncherStore((s) => s.clients)
  const running = clients.filter((client) => client.status === 'running').length

  return (
    <div className="flex items-center gap-2 px-3 py-1.5 rounded-full border border-white/[0.06] bg-zinc-900/50 shadow-glass animate-fade-rise">
      <StatusDot status={launching ? 'warning' : 'ok'} pulse />
      <span className="text-[11px] text-zinc-300 font-medium truncate max-w-[220px]">
        {launching ? 'Iniciando...' : 'En juego'}
        {clients.length === 1
          ? ` · ${clients[0].serverName}`
          : ` · ${running}/${clients.length} clientes`}
      </span>
    </div>
  )
}

export function AppHeader() {
  const ingame = useUiModeStore((s) => s.mode === 'ingame')

  return (
    <header className="shrink-0 flex items-end justify-between px-4 py-2.5 border-b border-white/[0.06] bg-zinc-950/60 backdrop-blur-sm">
      <div>
        <h1 className="text-xl font-bold tracking-tight">
          <span className="text-amber-400">RO</span>
          <span className="text-zinc-100">-Launcher</span>
        </h1>
        <p className="text-xs text-zinc-500 mt-0.5">Ragnarok Online · Linux</p>
      </div>
      {ingame ? (
        <IngameStatusChip />
      ) : (
        <p className="text-[11px] text-zinc-600 tracking-wide">
          Developed by: <span className="text-zinc-400 font-medium">nndsk</span>
        </p>
      )}
    </header>
  )
}
