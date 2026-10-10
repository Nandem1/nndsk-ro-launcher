import { useState } from 'react'
import { Pencil, Plus, X } from 'lucide-react'
import { AddServerModal } from './AddServerModal'
import { EditServerModal } from './EditServerModal'
import { useServersStore } from './servers.store'
import { Panel } from '../../shared/ui/Panel'
import { IconButton } from '../../shared/ui/Button'
import type { ServerConfig } from '../../shared/types'

export function ServerList() {
  const {
    servers,
    selectedId,
    loading,
    error,
    selectServer,
    removeServer,
    loadServers,
    clearError,
  } = useServersStore()
  const [showAdd, setShowAdd] = useState(false)
  const [editingServer, setEditingServer] = useState<ServerConfig | null>(null)

  const handleOpenAdd = () => {
    clearError()
    setShowAdd(true)
  }

  const handleOpenEdit = (server: ServerConfig) => {
    clearError()
    selectServer(server.id)
    setEditingServer(server)
  }

  return (
    <>
      <Panel
        title="Servidor"
        className="shrink-0"
        action={
          <IconButton
            label="Agregar servidor"
            variant="ghost"
            size="xs"
            onClick={handleOpenAdd}
          >
            <Plus className="w-3.5 h-3.5" />
          </IconButton>
        }
      >
        {loading && (
          <p className="text-muted text-sm py-1 text-center">
            Cargando servidores...
          </p>
        )}

        {error && !loading && (
          <div className="flex flex-col gap-2 mb-2 px-1">
            <p className="text-xs text-bad leading-relaxed">{error}</p>
            <button
              type="button"
              onClick={() => void loadServers()}
              className="text-xs text-muted hover:text-accent transition-colors self-start"
            >
              Reintentar
            </button>
          </div>
        )}

        {!loading && servers.length === 0 && !error && (
          <p className="text-muted text-sm py-1 text-center">
            Sin servidores — agrega uno con +
          </p>
        )}

        <div className="flex flex-col gap-0.5 -mx-1">
          {servers.map((server) => (
            <div
              key={server.id}
              className={`flex items-center gap-1 px-2 rounded-control transition-colors group
                ${selectedId === server.id ? 'bg-panel-raised border-t border-accent/50' : 'hover:bg-panel-raised border-t border-line'}`}
            >
              <label className="min-w-0 flex-1 flex items-center gap-3 py-2.5 cursor-pointer">
                <input
                  type="radio"
                  name="server"
                  value={server.id}
                  checked={selectedId === server.id}
                  onChange={() => selectServer(server.id)}
                  className="accent-accent w-3.5 h-3.5 shrink-0"
                />
                <span
                  className={`text-sm flex-1 truncate ${selectedId === server.id ? 'text-accent font-medium' : 'text-ink'}`}
                >
                  {server.name}
                </span>
              </label>
              <div className="flex items-center gap-0.5 opacity-50 group-hover:opacity-100 group-focus-within:opacity-100 transition-colors">
                <IconButton
                  label={`Editar ${server.name}`}
                  variant="ghost"
                  size="xs"
                  onClick={() => handleOpenEdit(server)}
                  className="text-muted hover:text-accent"
                >
                  <Pencil className="w-3 h-3" />
                </IconButton>
                <IconButton
                  label={`Eliminar ${server.name}`}
                  variant="ghost"
                  size="xs"
                  onClick={() => void removeServer(server.id)}
                  className="text-muted hover:text-bad"
                >
                  <X className="w-3.5 h-3.5" />
                </IconButton>
              </div>
            </div>
          ))}
        </div>
      </Panel>

      {showAdd && <AddServerModal onClose={() => setShowAdd(false)} />}
      {editingServer && (
        <EditServerModal
          server={editingServer}
          onClose={() => setEditingServer(null)}
        />
      )}
    </>
  )
}
