import { create } from 'zustand'
import { api } from '../../shared/api'
import { toErrorMessage } from '../../shared/errors'
import type { CreateInstallation, InstallationDraft } from '../../shared/types'

interface InstallationsState {
  drafts: InstallationDraft[]
  error: string | null
  load: () => Promise<void>
  create: (input: CreateInstallation) => Promise<InstallationDraft>
  start: (id: string) => Promise<void>
  cancel: (id: string) => Promise<void>
}

let loadGeneration = 0

export const useInstallationsStore = create<InstallationsState>((set) => {
  const upsert = (draft: InstallationDraft) => {
    ++loadGeneration
    set((state) => ({
      drafts: state.drafts.some((item) => item.id === draft.id)
        ? state.drafts.map((item) => (item.id === draft.id ? draft : item))
        : [...state.drafts, draft],
      error: null,
    }))
  }
  return {
    drafts: [],
    error: null,
    load: async () => {
      const generation = ++loadGeneration
      try {
        const drafts = await api.listInstallations()
        if (generation === loadGeneration) set({ drafts, error: null })
      } catch (error) {
        if (generation === loadGeneration) set({ error: toErrorMessage(error) })
      }
    },
    create: async (input) => {
      const draft = await api.createInstallation(input)
      upsert(draft)
      return draft
    },
    start: async (id) => upsert(await api.startInstallation(id)),
    cancel: async (id) => upsert(await api.cancelInstallation(id)),
  }
})
