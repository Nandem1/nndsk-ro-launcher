import { api } from '../../shared/api'
import { useLauncherStore } from './launcher.store'

let refreshGeneration = 0

export async function refreshGameClients() {
  const generation = ++refreshGeneration
  const revision = useLauncherStore.getState().clientsRevision
  const clients = await api.listGameClients()
  const state = useLauncherStore.getState()
  if (generation === refreshGeneration && revision === state.clientsRevision) {
    state.setClients(clients)
  }
}
