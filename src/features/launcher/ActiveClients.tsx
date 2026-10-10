import { Square, XCircle } from 'lucide-react'
import { api } from '../../shared/api'
import { toErrorMessage } from '../../shared/errors'
import { IconButton } from '../../shared/ui/Button'
import { Panel } from '../../shared/ui/Panel'
import { StatusDot } from '../../shared/ui/StatusDot'
import { useLogsStore } from '../logs/logs.store'
import { useLauncherStore } from './launcher.store'
import { refreshGameClients } from './refreshGameClients'

const statusLabels = {
  launching: 'Iniciando',
  running: 'En juego',
  stopping: 'Cerrando',
} as const

export function ActiveClients() {
  const clients = useLauncherStore((s) => s.clients)
  const setClientStatus = useLauncherStore((s) => s.setClientStatus)
  const setError = useLauncherStore((s) => s.setError)
  const addGameLog = useLogsStore((s) => s.addGameLog)

  if (clients.length === 0) return null

  const refresh = async () => {
    try {
      await refreshGameClients()
    } catch {
      addGameLog('No se pudo actualizar la lista de clientes')
    }
  }

  const stopClient = async (clientId: string) => {
    setClientStatus(clientId, 'stopping')
    try {
      await api.stopGame(clientId)
    } catch (error) {
      const message = toErrorMessage(error)
      setError(message)
      addGameLog(`Error al detener cliente: ${message}`)
      await refresh()
    }
  }

  const stopAll = async () => {
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
      await api.stopAllGames()
    } catch (error) {
      const message = toErrorMessage(error)
      setError(message)
      addGameLog(`Error al detener clientes: ${message}`)
      await refresh()
    }
  }

  return (
    <Panel
      title={`Clientes activos · ${clients.length}`}
      className="shrink-0"
      action={
        <button
          type="button"
          onClick={() => void stopAll()}
          className="inline-flex items-center gap-1 text-caption text-muted hover:text-bad transition-colors"
        >
          <XCircle className="w-3.5 h-3.5" />
          Detener todos
        </button>
      }
    >
      <div className="flex flex-col gap-1">
        {clients.map((client, index) => (
          <div key={client.clientId} className="flex items-center gap-2 py-2">
            <StatusDot
              status={client.status === 'running' ? 'ok' : 'warning'}
              pulse={client.status !== 'stopping'}
            />
            <div className="min-w-0 flex-1">
              <p className="truncate text-xs font-medium text-ink">
                {client.serverName} · Cliente {index + 1}
              </p>
              <p className="text-caption font-mono text-muted tabular-nums">
                {statusLabels[client.status]}
                {client.pid ? ` · PID ${client.pid}` : ''}
              </p>
            </div>
            <IconButton
              label={`Detener ${client.serverName} cliente ${index + 1}`}
              variant="primary"
              tone="bad"
              size="xs"
              disabled={client.status === 'stopping'}
              onClick={() => void stopClient(client.clientId)}
            >
              <Square className="w-3 h-3" />
            </IconButton>
          </div>
        ))}
      </div>
      {clients.length > 1 && (
        <p className="mt-2 text-caption leading-relaxed text-muted">
          AutoPot, AutoBuff y Spammer se habilitan nuevamente cuando quede un
          solo cliente.
        </p>
      )}
    </Panel>
  )
}
