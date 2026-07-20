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
          <p className="text-zinc-600 text-sm py-1 text-center">
            Cargando servidores...
          </p>
        )}

        {error && !loading && (
          <div className="flex flex-col gap-2 mb-2 px-1">
            <p className="text-xs text-red-400 leading-relaxed">{error}</p>
            <button
              type="button"
              onClick={() => void loadServers()}
              className="text-xs text-zinc-500 hover:text-amber-400 transition-colors self-start"
            >
              Reintentar
            </button>
          </div>
        )}

        {!loading && servers.length === 0 && !error && (
          <p className="text-zinc-600 text-sm py-1 text-center">
            Sin servidores — agrega uno con +
          </p>
        )}

        <div className="flex flex-col gap-0.5 -mx-1">
          {servers.map((server) => (
            <div
              key={server.id}
              className={`flex items-center gap-1 px-2 rounded-lg transition-colors group
                ${selectedId === server.id ? 'bg-amber-500/10 border border-amber-500/20 shadow-glow-amber' : 'hover:bg-zinc-800/60 border border-transparent'}`}
            >
              <label className="min-w-0 flex-1 flex items-center gap-3 py-2.5 cursor-pointer">
                <input
                  type="radio"
                  name="server"
                  value={server.id}
                  checked={selectedId === server.id}
                  onChange={() => selectServer(server.id)}
                  className="accent-amber-500 w-3.5 h-3.5 shrink-0"
                />
                <span
                  className={`text-sm flex-1 truncate ${selectedId === server.id ? 'text-amber-100 font-medium' : 'text-zinc-200'}`}
                >
                  {server.name}
                </span>
              </label>
              <div className="flex items-center gap-0.5 opacity-50 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity">
                <IconButton
                  label={`Editar ${server.name}`}
                  variant="ghost"
                  size="xs"
                  onClick={() => handleOpenEdit(server)}
                  className="text-zinc-600 hover:text-amber-300"
                >
                  <Pencil className="w-3 h-3" />
                </IconButton>
                <IconButton
                  label={`Eliminar ${server.name}`}
                  variant="ghost"
                  size="xs"
                  onClick={() => void removeServer(server.id)}
                  className="text-zinc-700 hover:text-red-400"
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
