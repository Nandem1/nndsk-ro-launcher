import { create } from 'zustand'
import { api } from '../../shared/api'
import { runSafely } from '../../shared/async'
import type { AdvancedDepsStatus } from '../../shared/types'
import type { RunnerInfo } from '../../shared/types'
import { advancedStatusFromDeps } from './advanced.logic'
import { resolveRunnerAfterLoad } from './settings.logic'

interface SettingsState {
  runners: RunnerInfo[]
  selectedRunner: string
  advancedStatus: AdvancedDepsStatus | null
  prefixConfigured: boolean
  init: () => Promise<void>
  loadSettings: () => Promise<void>
  loadRunners: () => Promise<void>
  loadDepsStatus: (runner: string) => Promise<void>
  setRunner: (path: string) => Promise<void>
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  runners: [],
  selectedRunner: '',
  advancedStatus: null,
  prefixConfigured: false,

  init: async () => {
    await get().loadSettings()
    await get().loadRunners()
  },

  loadSettings: async () => {
    const settings = await api.loadSettings()
    set({ selectedRunner: settings.defaultRunner })
  },

  loadRunners: async () => {
    const runners = await api.listRunners()
    set({ runners })

    const resolution = resolveRunnerAfterLoad(get().selectedRunner, runners)
    if (!resolution) return

    if (resolution.persist) {
      const result = await runSafely(() =>
        api.saveSettings({ defaultRunner: resolution.path }),
      )
      if (!result.ok) return
    }

    set({ selectedRunner: resolution.path })
    await get().loadDepsStatus(resolution.path)
  },

  loadDepsStatus: async (runner: string) => {
    const result = await runSafely(() => api.checkDependencies(runner))
    set({
      advancedStatus: result.ok ? advancedStatusFromDeps(result.value) : null,
      prefixConfigured: result.ok ? result.value.prefixConfigured : false,
    })
  },

  setRunner: async (path) => {
    const previous = get().selectedRunner
    const result = await runSafely(() => api.saveSettings({ defaultRunner: path }))
    if (!result.ok) {
      set({ selectedRunner: previous })
      return
    }
    set({ selectedRunner: path })
    await get().loadDepsStatus(path)
  },
}))
