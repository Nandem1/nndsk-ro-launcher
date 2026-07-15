import { create } from 'zustand'
import { api } from '../../shared/api'
import { runSafely } from '../../shared/async'
import { persistWithError } from '../../shared/store/persist'
import type { ServerConfig } from '../../shared/types'
import {
  findSelectedServer,
  firstServerId,
  nextSelectedId,
} from './servers.logic'

interface ServersState {
  servers: ServerConfig[]
  selectedId: string | null
  loading: boolean
  error: string | null
  loadServers: () => Promise<void>
  selectServer: (id: string) => void
  addServer: (server: ServerConfig) => Promise<void>
  removeServer: (id: string) => Promise<void>
  updateServer: (id: string, patch: Partial<ServerConfig>) => Promise<void>
  clearError: () => void
  getSelected: () => ServerConfig | null
}

export const useServersStore = create<ServersState>((set, get) => ({
  servers: [],
  selectedId: null,
  loading: true,
  error: null,

  loadServers: async () => {
    set({ loading: true, error: null })
    const result = await runSafely(() => api.listServers())
    if (result.ok) {
      set({
        servers: result.value,
        selectedId: firstServerId(result.value),
        loading: false,
      })
      return
    }
    set({ loading: false, error: result.error })
  },

  selectServer: (id) => set({ selectedId: id }),

  addServer: async (server) => {
    const updated = [...get().servers, server]
    const ok = await persistWithError(
      (error) => set({ error }),
      () => api.saveServers(updated),
      { rethrow: true },
    )
    if (ok) set({ servers: updated, selectedId: server.id })
  },

  removeServer: async (id) => {
    const { servers, selectedId } = get()
    const updated = servers.filter((s) => s.id !== id)
    const ok = await persistWithError(
      (error) => set({ error }),
      () => api.saveServers(updated),
    )
    if (!ok) return
    set({
      servers: updated,
      selectedId: nextSelectedId(selectedId, id, updated),
    })
  },

  updateServer: async (id, patch) => {
    const updated = get().servers.map((s) =>
      s.id === id ? { ...s, ...patch } : s,
    )
    const ok = await persistWithError(
      (error) => set({ error }),
      () => api.saveServers(updated),
    )
    if (ok) set({ servers: updated })
  },

  clearError: () => set({ error: null }),

  getSelected: () => findSelectedServer(get().servers, get().selectedId),
}))
