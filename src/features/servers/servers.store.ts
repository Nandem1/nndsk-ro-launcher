import { invoke } from '@tauri-apps/api/core'
import { create } from 'zustand'
import type { ServerConfig } from './servers.config'

interface ServersState {
  servers: ServerConfig[]
  selectedId: string | null
  loading: boolean
  loadServers: () => Promise<void>
  selectServer: (id: string) => void
  addServer: (server: ServerConfig) => Promise<void>
  removeServer: (id: string) => Promise<void>
  getSelected: () => ServerConfig | null
}

export const useServersStore = create<ServersState>((set, get) => ({
  servers: [],
  selectedId: null,
  loading: true,

  loadServers: async () => {
    const servers = await invoke<ServerConfig[]>('list_servers')
    set({ servers, selectedId: servers[0]?.id ?? null, loading: false })
  },

  selectServer: (id) => set({ selectedId: id }),

  addServer: async (server) => {
    const updated = [...get().servers, server]
    await invoke('save_servers', { servers: updated })
    set({ servers: updated, selectedId: server.id })
  },

  removeServer: async (id) => {
    const { servers, selectedId } = get()
    const updated = servers.filter((s) => s.id !== id)
    await invoke('save_servers', { servers: updated })
    set({
      servers: updated,
      selectedId: selectedId === id ? (updated[0]?.id ?? null) : selectedId,
    })
  },

  getSelected: () => {
    const { servers, selectedId } = get()
    return servers.find((s) => s.id === selectedId) ?? null
  },
}))
