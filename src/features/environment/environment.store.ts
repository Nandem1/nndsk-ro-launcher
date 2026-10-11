import { create } from 'zustand'

interface EnvironmentState {
  helpContext: string | null
  openHelp: (context: string) => void
  closeHelp: () => void
}

export const useEnvironmentStore = create<EnvironmentState>((set) => ({
  helpContext: null,
  openHelp: (context) => set({ helpContext: context }),
  closeHelp: () => set({ helpContext: null }),
}))
