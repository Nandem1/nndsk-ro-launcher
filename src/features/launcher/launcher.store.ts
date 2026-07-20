import { create } from 'zustand'
import type { ProgressPayload } from '../../shared/types'

export type LaunchStatus =
  'idle' | 'checking' | 'setting-up' | 'launching' | 'running' | 'error'

interface LauncherState {
  status: LaunchStatus
  setupProgress: ProgressPayload | null
  error: string | null
  setStatus: (status: LaunchStatus) => void
  setProgress: (progress: ProgressPayload | null) => void
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

export function isLauncherBusy(status: LaunchStatus): boolean {
  return (
    status === 'checking' ||
    status === 'setting-up' ||
    status === 'launching' ||
    status === 'running'
  )
}
