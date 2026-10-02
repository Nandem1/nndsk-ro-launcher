import { create } from 'zustand'
import type {
  GameClientSnapshot,
  GameClientStatus,
  ProgressPayload,
} from '../../shared/types'

export type LaunchStatus =
  'idle' | 'checking' | 'setting-up' | 'launching' | 'error'

export interface LauncherState {
  status: LaunchStatus
  clients: GameClientSnapshot[]
  clientsRevision: number
  closedClientIds: string[]
  setupProgress: ProgressPayload | null
  operationId: string | null
  error: string | null
  setStatus: (status: LaunchStatus) => void
  setClients: (clients: GameClientSnapshot[]) => void
  upsertClient: (client: GameClientSnapshot) => void
  removeClient: (clientId: string) => void
  setClientStatus: (clientId: string, status: GameClientStatus) => void
  setProgress: (progress: ProgressPayload | null) => void
  setError: (error: string | null) => void
}

export const useLauncherStore = create<LauncherState>((set) => ({
  status: 'idle',
  clients: [],
  clientsRevision: 0,
  closedClientIds: [],
  setupProgress: null,
  operationId: null,
  error: null,
  setStatus: (status) => set({ status }),
  setClients: (clients) =>
    set((state) => ({
      clients: clients.filter(
        (client) => !state.closedClientIds.includes(client.clientId),
      ),
      clientsRevision: state.clientsRevision + 1,
    })),
  upsertClient: (client) =>
    set((state) => {
      if (state.closedClientIds.includes(client.clientId)) return state
      const index = state.clients.findIndex(
        (candidate) => candidate.clientId === client.clientId,
      )
      if (index < 0)
        return {
          clients: [...state.clients, client],
          clientsRevision: state.clientsRevision + 1,
        }
      const clients = [...state.clients]
      clients[index] = client
      return { clients, clientsRevision: state.clientsRevision + 1 }
    }),
  removeClient: (clientId) =>
    set((state) => ({
      clients: state.clients.filter((client) => client.clientId !== clientId),
      clientsRevision: state.clientsRevision + 1,
      closedClientIds: [...new Set([...state.closedClientIds, clientId])],
    })),
  setClientStatus: (clientId, status) =>
    set((state) => ({
      clientsRevision: state.clientsRevision + 1,
      clients: state.clients.map((client) =>
        client.clientId === clientId ? { ...client, status } : client,
      ),
    })),
  setProgress: (setupProgress) => set({ setupProgress }),
  setError: (error) => set({ error }),
}))

export function isLauncherBusy(status: LaunchStatus): boolean {
  return (
    status === 'checking' || status === 'setting-up' || status === 'launching'
  )
}

export function isSoleRunningClientForServer(
  state: Pick<LauncherState, 'clients'>,
  serverId: string | null | undefined,
): boolean {
  return (
    !!serverId &&
    state.clients.length === 1 &&
    state.clients[0].serverId === serverId &&
    state.clients[0].status === 'running'
  )
}
