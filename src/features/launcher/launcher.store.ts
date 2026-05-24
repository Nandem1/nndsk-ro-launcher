import { create } from 'zustand'

export type LaunchStatus = 'idle' | 'setting-up' | 'launching' | 'running' | 'error'

interface LauncherState {
  status: LaunchStatus
  setupProgress: { step: string; percent: number } | null
  error: string | null
  setStatus: (status: LaunchStatus) => void
  setProgress: (progress: { step: string; percent: number } | null) => void
  setError: (error: string | null) => void
}

export const useLauncherStore = create<LauncherState>((set) => ({
  status: 'idle',
  setupProgress: null,
  error: null,
  setStatus: (status) => set({ status }),
  setProgress: (setupProgress) => set({ setupProgress }),
  setError: (error) => set({ error }),
}))
