import { ChevronsRight, Square } from 'lucide-react'
import { IconButton } from '../../shared/ui/Button'
import { StatusDot } from '../../shared/ui/StatusDot'
import { useSelectedServer } from '../servers/useSelectedServer'
import { useUiModeStore } from '../../app/uiMode.store'
import { useLauncherStore } from './launcher.store'
import { useLaunchGame } from './useLaunchGame'

export function IngameRail() {
  const server = useSelectedServer()
  const launching = useLauncherStore((s) => s.status === 'launching')
  const toggleRailPeek = useUiModeStore((s) => s.toggleRailPeek)
  const { handleStop } = useLaunchGame(server)

  const initial = server?.name.trim().charAt(0).toUpperCase() || '?'

  return (
    <section className="h-full rounded-xl border border-white/[0.06] bg-gradient-to-b from-zinc-800/30 to-zinc-900/50 backdrop-blur-sm shadow-glass flex flex-col items-center py-3 gap-3 animate-rail-collapse">
      <div
        className="relative w-10 h-10 rounded-xl border border-white/[0.08] bg-zinc-950/50 shadow-glass flex items-center justify-center"
        title={server?.name}
      >
        <span className="text-sm font-bold text-amber-200/90">{initial}</span>
        <span className="absolute -top-0.5 -right-0.5">
          <StatusDot status={launching ? 'warning' : 'ok'} pulse />
        </span>
      </div>

      <IconButton
        label="Ver panel"
        variant="ghost"
        size="md"
        onClick={toggleRailPeek}
      >
        <ChevronsRight className="w-4 h-4" />
      </IconButton>

      <IconButton
        label="Detener juego"
        variant="danger"
        size="lg"
        className="mt-auto"
        onClick={handleStop}
      >
        <Square className="w-4 h-4" />
      </IconButton>
    </section>
  )
}
