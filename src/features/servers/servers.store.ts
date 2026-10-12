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
  registerInstallation: (
    id: string,
    executablePath: string,
    patcherPath: string | null,
  ) => Promise<void>
  removeServer: (id: string) => Promise<void>
  updateServer: (
    id: string,
    update: ServerUpdater,
  ) => Promise<ServerConfig | null>
  retryPersistence: () => Promise<boolean>
  clearError: () => void
  getSelected: () => ServerConfig | null
}

let diskTail: Promise<void> | null = null

/** Confirmation and snapshot writes share one queue. A stale write cannot remove a
 * server inserted by the backend, even if the modal disappears during confirmation. */
function serializeDiskOperation<T>(operation: () => Promise<T>): Promise<T> {
  const work = diskTail ? diskTail.then(operation) : operation()
  const settled = work.then(
    () => {},
    () => {},
  )
  diskTail = settled
  void settled.then(() => {
    if (diskTail === settled) diskTail = null
  })
  return work
}

const persistence = new LatestSnapshotWriter<ServerConfig[]>(() =>
  serializeDiskOperation(() =>
    api.saveServers(useServersStore.getState().servers),
  ),
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

  registerInstallation: async (id, executablePath, patcherPath) => {
    // Drain earlier snapshot writes before the backend inserts the completed server.
    await persistence.write(get().servers)
    await serializeDiskOperation(async () => {
      let server: ServerConfig
      try {
        server = await api.confirmInstallation(id, executablePath, patcherPath)
      } catch (error) {
        // The backend saves the server before completing the draft. Recover that
        // durable insert before queued local edits can overwrite the server file.
        const recovered = await runSafely(() => api.listServers())
        const installed = recovered.ok
          ? recovered.value.find(
              (item) => item.installation?.installationId === id,
            )
          : undefined
        if (
          installed &&
          !get().servers.some((item) => item.id === installed.id)
        ) {
          ++editGeneration
          set((state) => ({ servers: [...state.servers, installed] }))
        }
        throw error
      }
      ++editGeneration
      set((state) => ({
        servers: state.servers.some((item) => item.id === server.id)
          ? state.servers.map((item) => {
              if (item.id !== server.id) return item
              return item.installation?.installationId ===
                server.installation?.installationId
                ? item
                : server
            })
          : [...state.servers, server],
        selectedId: server.id,
        error: null,
      }))
    })
    // Merge edits made while confirmation was in flight into the newest snapshot.
    await persistence.write(get().servers)
  },

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
