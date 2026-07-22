import { ChevronsRight, Square } from 'lucide-react'
import { IconButton } from '../../shared/ui/Button'
import { StatusDot } from '../../shared/ui/StatusDot'
import { useUiModeStore } from '../../app/uiMode.store'
import { useLauncherStore } from './launcher.store'
import { api } from '../../shared/api'
import { toErrorMessage } from '../../shared/errors'

export function IngameRail() {
  const clients = useLauncherStore((s) => s.clients)
  const launching = useLauncherStore((s) => s.status === 'launching')
  const setError = useLauncherStore((s) => s.setError)
  const setClients = useLauncherStore((s) => s.setClients)
  const setClientStatus = useLauncherStore((s) => s.setClientStatus)
  const toggleRailPeek = useUiModeStore((s) => s.toggleRailPeek)

  const initial = clients[0]?.serverName.trim().charAt(0).toUpperCase() || '?'

  const handleStop = async () => {
    if (clients.length === 0) return
    if (
      clients.length > 1 &&
      !window.confirm(`¿Detener los ${clients.length} clientes abiertos?`)
    ) {
      return
    }
    for (const client of clients) {
      setClientStatus(client.clientId, 'stopping')
    }
    try {
      if (clients.length === 1) {
        await api.stopGame(clients[0].clientId)
      } else {
        await api.stopAllGames()
      }
    } catch (error) {
      setError(toErrorMessage(error))
      try {
        setClients(await api.listGameClients())
      } catch {
        // El evento de salida o la próxima apertura volverán a sincronizar.
      }
    }
  }

  return (
    <section className="h-full rounded-xl border border-white/[0.06] bg-gradient-to-b from-zinc-800/30 to-zinc-900/50 backdrop-blur-sm shadow-glass flex flex-col items-center py-3 gap-3 animate-rail-collapse">
      <div
        className="relative w-10 h-10 rounded-xl border border-white/[0.08] bg-zinc-950/50 shadow-glass flex items-center justify-center"
        title={`${clients.length} cliente${clients.length === 1 ? '' : 's'} activo${clients.length === 1 ? '' : 's'}`}
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
        label={clients.length > 1 ? 'Detener todos' : 'Detener juego'}
        variant="danger"
        size="lg"
        className="mt-auto"
        onClick={() => void handleStop()}
      >
        <Square className="w-4 h-4" />
      </IconButton>
    </section>
  )
}
