import { create } from 'zustand'
import { api } from '../../shared/api'
import { runSafely } from '../../shared/async'
import type {
  AdvancedDepsStatus,
  DependencyStatus,
  RunnerInfo,
  ServerConfig,
  StorageNotice,
} from '../../shared/types'
import { advancedStatusFromDeps } from './advanced.logic'
import { resolveRunnerAfterLoad } from './settings.logic'
import { runtimeStatusKey } from '../../shared/resolveRunner'

interface SettingsState {
  runners: RunnerInfo[]
  selectedRunner: string
  advancedStatus: AdvancedDepsStatus | null
  advancedStatusKey: string | null
  loading: boolean
  savingRunner: boolean
  error: string | null
  notice: StorageNotice | null
  init: () => Promise<boolean>
  loadSettings: () => Promise<void>
  loadRunners: () => Promise<void>
  loadDepsStatus: (
    runner: string,
    server?: ServerConfig | null,
  ) => Promise<void>
  applyDepsStatus: (status: DependencyStatus, key: string) => void
  setRunner: (path: string) => Promise<void>
}

let depsRequestId = 0
let runnerSaveRequestId = 0
let runnerSaveTail: Promise<void> = Promise.resolve()
let lastPersistedRunner = ''

export const useSettingsStore = create<SettingsState>((set, get) => ({
  runners: [],
  selectedRunner: '',
  advancedStatus: null,
  advancedStatusKey: null,
  loading: true,
  savingRunner: false,
  error: null,
  notice: null,

  init: async () => {
    set({ loading: true, error: null, notice: null })
    const result = await runSafely(async () => {
      await get().loadSettings()
      await get().loadRunners()
    })
    set({ loading: false, error: result.ok ? null : result.error })
    return result.ok
  },

  loadSettings: async () => {
    const settings = await api.loadSettings()
    lastPersistedRunner = settings.defaultRunner
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
      if (!result.ok) {
        set({ error: result.error })
        throw new Error(result.error)
      }
      lastPersistedRunner = resolution.path
      set({
        notice: {
          source: 'settings',
          kind: 'migrated',
          message: 'El runtime fue migrado al entorno Ragnarok administrado',
        },
      })
    }

    set({ selectedRunner: resolution.path })
    await get().loadDepsStatus(resolution.path)
  },

  loadDepsStatus: async (runner: string, server = null) => {
    const requestId = ++depsRequestId
    const key = runtimeStatusKey(server, runner)
    set({ advancedStatus: null, advancedStatusKey: null })
    const result = await runSafely(() =>
      api.checkDependencies(server, runner || null),
    )
    if (requestId !== depsRequestId) return
    set({
      advancedStatus: result.ok ? advancedStatusFromDeps(result.value) : null,
      advancedStatusKey: result.ok ? key : null,
    })
  },

  applyDepsStatus: (status, key) => {
    ++depsRequestId
    set({
      advancedStatus: advancedStatusFromDeps(status),
      advancedStatusKey: key,
    })
  },

  setRunner: async (path) => {
    const requestId = ++runnerSaveRequestId
    set({ savingRunner: true, error: null })

    const save = async () => {
      const result = await runSafely(() =>
        api.saveSettings({ defaultRunner: path }),
      )
      if (result.ok) lastPersistedRunner = path
      if (requestId !== runnerSaveRequestId) return

      set({
        selectedRunner: result.ok ? path : lastPersistedRunner,
        savingRunner: false,
        error: result.ok ? null : result.error,
      })
    }
    const queued = runnerSaveTail.then(save, save)
    runnerSaveTail = queued.catch(() => undefined)
    await queued
  },
}))
