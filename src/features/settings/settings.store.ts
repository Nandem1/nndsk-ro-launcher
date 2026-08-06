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
  richPresenceEnabled: boolean
  advancedStatus: AdvancedDepsStatus | null
  advancedStatusKey: string | null
  loading: boolean
  savingRunner: boolean
  savingPresence: boolean
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
  setRichPresenceEnabled: (enabled: boolean) => Promise<void>
}

let depsRequestId = 0
let runnerSaveRequestId = 0
let presenceSaveRequestId = 0
let settingsSaveTail: Promise<unknown> = Promise.resolve()
let lastPersistedRunner = ''

export const useSettingsStore = create<SettingsState>((set, get) => ({
  runners: [],
  selectedRunner: '',
  richPresenceEnabled: false,
  advancedStatus: null,
  advancedStatusKey: null,
  loading: true,
  savingRunner: false,
  savingPresence: false,
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
    set({
      selectedRunner: settings.defaultRunner,
      richPresenceEnabled: settings.richPresenceEnabled ?? false,
    })
  },

  loadRunners: async () => {
    const runners = await api.listRunners()
    set({ runners })

    const resolution = resolveRunnerAfterLoad(get().selectedRunner, runners)
    if (!resolution) return

    if (resolution.persist) {
      const result = await runSafely(() =>
        api.saveSettings({
          defaultRunner: resolution.path,
          richPresenceEnabled: get().richPresenceEnabled,
        }),
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
        api.saveSettings({
          defaultRunner: path,
          richPresenceEnabled: get().richPresenceEnabled,
        }),
      )
      if (result.ok) lastPersistedRunner = path
      if (requestId !== runnerSaveRequestId) return

      set({
        selectedRunner: result.ok ? path : lastPersistedRunner,
        savingRunner: false,
        error: result.ok ? null : result.error,
      })
    }
    const queued = settingsSaveTail.then(save, save)
    settingsSaveTail = queued.catch(() => undefined)
    await queued
  },

  setRichPresenceEnabled: async (enabled) => {
    const requestId = ++presenceSaveRequestId
    const previous = get().richPresenceEnabled
    set({ richPresenceEnabled: enabled, savingPresence: true, error: null })

    const save = async () =>
      runSafely(() =>
        api.saveSettings({
          defaultRunner: get().selectedRunner,
          richPresenceEnabled: enabled,
        }),
      )
    const queued = settingsSaveTail.then(save, save)
    settingsSaveTail = queued.catch(() => undefined)
    const result = await queued
    if (requestId !== presenceSaveRequestId) return

    set({
      richPresenceEnabled: result.ok ? enabled : previous,
      savingPresence: false,
      error: result.ok ? null : result.error,
    })
  },
}))
