import { ChevronsRight, Square } from 'lucide-react'
import { IconButton } from '../../shared/ui/Button'
import { StatusDot } from '../../shared/ui/StatusDot'
import { useUiModeStore } from '../../app/uiMode.store'
import { useLauncherStore } from './launcher.store'
import { api } from '../../shared/api'
import { toErrorMessage } from '../../shared/errors'
import { refreshGameClients } from './refreshGameClients'

export function IngameRail() {
  const clients = useLauncherStore((s) => s.clients)
  const launching = useLauncherStore((s) => s.status === 'launching')
  const setError = useLauncherStore((s) => s.setError)
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
        await refreshGameClients()
      } catch {
        // El evento de salida o la próxima apertura volverán a sincronizar.
      }
    }
  }

  return (
    <section className="h-full rounded-panel border border-overlay-light/[0.06] bg-panel-gradient from-panel-raised/30 to-panel/50 backdrop-blur-panel shadow-panel flex flex-col items-center py-3 gap-3 animate-rail-collapse">
      <div
        className="relative w-10 h-10 rounded-panel border border-overlay-light/[0.08] bg-surface/50 shadow-panel flex items-center justify-center"
        title={`${clients.length} cliente${clients.length === 1 ? '' : 's'} activo${clients.length === 1 ? '' : 's'}`}
      >
        <span className="text-sm font-bold text-accent-soft/90">{initial}</span>
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
        variant="primary"
        tone="bad"
        size="lg"
        className="mt-auto"
        onClick={() => void handleStop()}
      >
        <Square className="w-4 h-4" />
      </IconButton>
    </section>
  )
}
