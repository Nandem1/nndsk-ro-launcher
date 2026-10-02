import { create } from 'zustand'
import { api } from '../../shared/api'
import { runSafely } from '../../shared/async'
import { toErrorMessage } from '../../shared/errors'
import { LatestSnapshotWriter } from '../../shared/store/latestSnapshotWriter'
import type { ServerConfig } from '../../shared/types'
import {
  findSelectedServer,
  firstServerId,
  nextSelectedId,
} from './servers.logic'

export type ServerUpdater =
  Partial<ServerConfig> | ((server: ServerConfig) => ServerConfig)

interface ServersState {
  servers: ServerConfig[]
  selectedId: string | null
  loading: boolean
  error: string | null
  loadServers: () => Promise<boolean>
  selectServer: (id: string) => void
  addServer: (server: ServerConfig) => Promise<void>
  removeServer: (id: string) => Promise<void>
  updateServer: (
    id: string,
    update: ServerUpdater,
  ) => Promise<ServerConfig | null>
  retryPersistence: () => Promise<boolean>
  clearError: () => void
  getSelected: () => ServerConfig | null
}

const persistence = new LatestSnapshotWriter<ServerConfig[]>((servers) =>
  api.saveServers(servers),
)
let loadGeneration = 0
let editGeneration = 0

function applyUpdate(
  server: ServerConfig,
  update: ServerUpdater,
): ServerConfig {
  return typeof update === 'function'
    ? update(server)
    : { ...server, ...update }
}

export const useServersStore = create<ServersState>((set, get) => ({
  servers: [],
  selectedId: null,
  loading: true,
  error: null,

  loadServers: async () => {
    const generation = ++loadGeneration
    const edits = editGeneration
    set({ loading: true, error: null })
    const result = await runSafely(() => api.listServers())
    if (generation !== loadGeneration) return result.ok
    if (edits !== editGeneration) {
      set({ loading: false })
      return result.ok
    }
    if (result.ok) {
      set({
        servers: result.value,
        selectedId: result.value.some(
          (server) => server.id === get().selectedId,
        )
          ? get().selectedId
          : firstServerId(result.value),
        loading: false,
      })
      return true
    }
    set({ loading: false, error: result.error })
    return false
  },

  selectServer: (id) => set({ selectedId: id }),

  addServer: async (server) => {
    const generation = ++editGeneration
    const selection = get().selectedId
    const previous = get().servers
    const updated = [...previous, server]
    set({ servers: updated, error: null })
    try {
      await persistence.write(updated)
      if (generation === editGeneration && get().selectedId === selection)
        set({ selectedId: server.id })
    } catch (error) {
      set((state) => ({
        servers: state.servers.filter((item) => item.id !== server.id),
        error:
          generation === editGeneration ? toErrorMessage(error) : state.error,
      }))
      throw error
    }
  },

  removeServer: async (id) => {
    const generation = ++editGeneration
    const { servers, selectedId } = get()
    const updated = servers.filter((s) => s.id !== id)
    set({
      servers: updated,
      selectedId: nextSelectedId(selectedId, id, updated),
      error: null,
    })
    try {
      await persistence.write(updated)
    } catch (error) {
      if (generation === editGeneration)
        set({ servers, selectedId, error: toErrorMessage(error) })
    }
  },

  updateServer: async (id, update) => {
    const generation = ++editGeneration
    let nextServer: ServerConfig | null = null
    const updated = get().servers.map((server) => {
      if (server.id !== id) return server
      nextServer = applyUpdate(server, update)
      return nextServer
    })
    if (!nextServer) return null

    set({ servers: updated, error: null })
    try {
      await persistence.write(updated)
      return nextServer
    } catch (error) {
      if (generation === editGeneration) set({ error: toErrorMessage(error) })
      return null
    }
  },

  retryPersistence: async () => {
    try {
      await persistence.write(get().servers)
      set({ error: null })
      return true
    } catch (error) {
      set({ error: toErrorMessage(error) })
      return false
    }
  },

  clearError: () => set({ error: null }),

  getSelected: () => findSelectedServer(get().servers, get().selectedId),
}))
