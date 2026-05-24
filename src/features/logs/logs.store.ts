import { create } from 'zustand'

const MAX_LOGS = 200

interface LogsState {
  logs: string[]
  addLog: (line: string) => void
  clearLogs: () => void
}

export const useLogsStore = create<LogsState>((set) => ({
  logs: [],
  addLog: (line) =>
    set((state) => {
      const last = state.logs[state.logs.length - 1]
      if (last === line) return state
      return { logs: [...state.logs.slice(-(MAX_LOGS - 1)), line] }
    }),
  clearLogs: () => set({ logs: [] }),
}))
