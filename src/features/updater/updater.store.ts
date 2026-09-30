import { create } from 'zustand'
import { api } from '../../shared/api'
import { toErrorMessage } from '../../shared/errors'
import type { UpdateSnapshot } from '../../shared/types'

interface UpdaterState {
  snapshot: UpdateSnapshot | null
  busy: boolean
  error: string | null
  applySnapshot: (snapshot: UpdateSnapshot) => void
  refresh: () => Promise<void>
  check: () => Promise<void>
  install: () => Promise<void>
  relaunch: () => Promise<void>
}

export const useUpdaterStore = create<UpdaterState>((set, get) => ({
  snapshot: null,
  busy: false,
  error: null,

  applySnapshot: (snapshot) => set({ snapshot, error: null }),

  refresh: async () => {
    try {
      const snapshot = await api.getUpdateSnapshot()
      set({ snapshot, error: null })
    } catch (error) {
      set({ error: toErrorMessage(error) })
    }
  },

  check: async () => {
    if (get().busy) return
    set({ busy: true, error: null })
    try {
      const snapshot = await api.checkForUpdate()
      set({ snapshot, busy: false })
    } catch (error) {
      set({ busy: false, error: toErrorMessage(error) })
    }
  },

  install: async () => {
    if (get().busy) return
    set({ busy: true, error: null })
    try {
      const snapshot = await api.installCheckedUpdate()
      set({ snapshot, busy: false })
    } catch (error) {
      set({ busy: false, error: toErrorMessage(error) })
    }
  },

  relaunch: async () => {
    if (get().busy) return
    set({ busy: true, error: null })
    try {
      await api.relaunchUpdatedApp()
    } catch (error) {
      set({ busy: false, error: toErrorMessage(error) })
    }
  },
}))
