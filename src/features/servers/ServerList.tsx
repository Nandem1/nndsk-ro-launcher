import { useState } from 'react'
import { AddServerModal } from './AddServerModal'
import { useServersStore } from './servers.store'
import { Panel } from '../../shared/ui/Panel'

export function ServerList() {
  const { servers, selectedId, selectServer, removeServer } = useServersStore()
  const [showAdd, setShowAdd] = useState(false)

  return (
    <>
      <Panel
        title="Servidor"
        className="shrink-0"
        action={
          <button
            onClick={() => setShowAdd(true)}
            className="text-zinc-500 hover:text-amber-400 transition-colors text-base leading-none w-5 h-5 flex items-center justify-center rounded hover:bg-zinc-800"
            title="Agregar servidor"
          >
            +
          </button>
        }
      >
        {servers.length === 0 && (
          <p className="text-zinc-600 text-sm py-1 text-center">
            Sin servidores — agrega uno con +
          </p>
        )}

        <div className="flex flex-col gap-0.5 -mx-1">
          {servers.map((server) => (
            <label
              key={server.id}
              className={`flex items-center gap-3 px-2 py-2.5 rounded-lg cursor-pointer transition-colors group
                ${selectedId === server.id ? 'bg-amber-500/10 border border-amber-500/20' : 'hover:bg-zinc-800/60 border border-transparent'}`}
            >
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
              <button
                onClick={(e) => {
                  e.preventDefault()
                  removeServer(server.id)
                }}
                className="text-zinc-700 hover:text-red-400 transition-colors opacity-0 group-hover:opacity-100 text-sm leading-none w-5 h-5 flex items-center justify-center rounded hover:bg-zinc-800"
                title="Eliminar"
              >
                ×
              </button>
            </label>
          ))}
        </div>
      </Panel>

      {showAdd && <AddServerModal onClose={() => setShowAdd(false)} />}
    </>
  )
}
