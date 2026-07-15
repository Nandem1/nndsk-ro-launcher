import { create } from 'zustand'

const MAX_LOGS = 200

function appendLog(existing: string[], line: string): string[] {
  return [...existing.slice(-(MAX_LOGS - 1)), line]
}

function makeAppender(
  set: (
    partial:
      | LogsState
      | Partial<LogsState>
      | ((state: LogsState) => LogsState | Partial<LogsState>),
  ) => void,
  key: 'gameLogs' | 'toolLogs',
) {
  return (line: string) =>
    set((s) => {
      const logs = s[key]
      if (logs[logs.length - 1] === line) return s
      return { [key]: appendLog(logs, line) }
    })
}

interface LogsState {
  gameLogs: string[]
  toolLogs: string[]
  addGameLog: (line: string) => void
  addToolLog: (line: string) => void
  clearGameLogs: () => void
  clearToolLogs: () => void
}

export const useLogsStore = create<LogsState>((set) => ({
  gameLogs: [],
  toolLogs: [],
  addGameLog: makeAppender(set, 'gameLogs'),
  addToolLog: makeAppender(set, 'toolLogs'),
  clearGameLogs: () => set({ gameLogs: [] }),
  clearToolLogs: () => set({ toolLogs: [] }),
}))
