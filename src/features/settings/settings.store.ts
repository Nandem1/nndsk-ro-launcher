import { invoke } from '@tauri-apps/api/core'
import { create } from 'zustand'

export interface RunnerInfo {
  id: string
  name: string
  path: string
}

interface SettingsState {
  runners: RunnerInfo[]
  selectedRunner: string
  loadSettings: () => Promise<void>
  loadRunners: () => Promise<void>
  setRunner: (path: string) => Promise<void>
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  runners: [],
  selectedRunner: '',

  loadSettings: async () => {
    const settings = await invoke<{ defaultRunner: string }>('load_settings')
    set({ selectedRunner: settings.defaultRunner })
  },

  loadRunners: async () => {
    const runners = await invoke<RunnerInfo[]>('list_runners')
    set({ runners })

    const current = get().selectedRunner
    const preferred = runners.find((r) => r.id === 'proton-proton-cachyos-slr')
    const fallback = runners[0]

    if (!current && fallback) {
      set({ selectedRunner: preferred?.path ?? fallback.path })
      return
    }

    // Migrate stale duplicate paths: prefer canonical proton if user had wine default
    if (current === '/usr/bin/wine' && preferred) {
      set({ selectedRunner: preferred.path })
      await invoke('save_settings', { settings: { defaultRunner: preferred.path } })
    }
  },

  setRunner: async (path) => {
    set({ selectedRunner: path })
    await invoke('save_settings', { settings: { defaultRunner: path } })
  },
}))
