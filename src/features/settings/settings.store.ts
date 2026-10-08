import { create } from 'zustand'
import { api } from '../../shared/api'
import { runSafely } from '../../shared/async'
import type {
  AdvancedDepsStatus,
  DependencyStatus,
  RunnerInfo,
  ServerConfig,
  StorageNotice,
  AppSettings,
} from '../../shared/types'
import { advancedStatusFromDeps } from './advanced.logic'
import { resolveRunnerAfterLoad } from './settings.logic'
import { runtimeStatusKey } from '../../shared/resolveRunner'
import { MANAGED_RUNTIME_ID } from '../../shared/constants'

interface SettingsState {
  runners: RunnerInfo[]
  selectedRunner: string
  richPresenceEnabled: boolean
  advancedStatus: AdvancedDepsStatus | null
  advancedStatusKey: string | null
  advancedStatusError: string | null
  loading: boolean
  savingRunner: boolean
  importingRuntime: boolean
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
  importRuntimeArchive: (archivePath: string) => Promise<boolean>
  setRichPresenceEnabled: (enabled: boolean) => Promise<void>
}

let depsRequestId = 0
let runnerSaveRequestId = 0
let presenceSaveRequestId = 0
let settingsSaveTail: Promise<unknown> = Promise.resolve()
let lastPersistedRunner = ''
let lastPersistedPresence = false
let initialization: Promise<boolean> | null = null
let settingsLoadId = 0
let runnersLoadId = 0

async function persistSettings(settings: AppSettings) {
  const result = await runSafely(() => api.saveSettings(settings))
  if (result.ok) {
    // Every write replaces the whole settings document, not just the initiating field.
    lastPersistedRunner = settings.defaultRunner
    lastPersistedPresence = settings.richPresenceEnabled ?? false
  }
  return result
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  runners: [],
  selectedRunner: '',
  richPresenceEnabled: false,
  advancedStatus: null,
  advancedStatusKey: null,
  advancedStatusError: null,
  loading: true,
  savingRunner: false,
  importingRuntime: false,
  savingPresence: false,
  error: null,
  notice: null,

  init: () => {
    if (initialization) return initialization
    const work = async () => {
      const runnerRevision = runnerSaveRequestId
      const presenceRevision = presenceSaveRequestId
      set({ loading: true, error: null, notice: null })
      const result = await runSafely(async () => {
        await get().loadSettings()
        await get().loadRunners()
      })
      set({ loading: false })
      if (
        runnerRevision === runnerSaveRequestId &&
        presenceRevision === presenceSaveRequestId
      )
        set({ error: result.ok ? null : result.error })
      return result.ok
    }
    initialization = work().finally(() => {
      initialization = null
    })
    return initialization
  },

  loadSettings: async () => {
    const requestId = ++settingsLoadId
    const runnerRevision = runnerSaveRequestId
    const presenceRevision = presenceSaveRequestId
    const writePending = get().savingRunner || get().savingPresence
    const result = await runSafely(() => api.loadSettings())
    if (requestId !== settingsLoadId || writePending) return
    if (
      runnerRevision !== runnerSaveRequestId ||
      presenceRevision !== presenceSaveRequestId
    )
      return
    if (!result.ok) throw new Error(result.error)
    const settings = result.value
    lastPersistedRunner = settings.defaultRunner
    lastPersistedPresence = settings.richPresenceEnabled ?? false
    set({
      selectedRunner: settings.defaultRunner,
      richPresenceEnabled: settings.richPresenceEnabled ?? false,
    })
  },

  loadRunners: async () => {
    const requestId = ++runnersLoadId
    const runnerRevision = runnerSaveRequestId
    const result = await runSafely(() => api.listRunners())
    if (requestId !== runnersLoadId) return
    if (!result.ok) {
      if (runnerRevision !== runnerSaveRequestId) return
      throw new Error(result.error)
    }
    const runners = result.value
    set({ runners })
    if (runnerRevision !== runnerSaveRequestId) return

    const resolution = resolveRunnerAfterLoad(get().selectedRunner, runners)
    if (!resolution) return

    if (resolution.persist) {
      const save = async () => {
        if (runnerRevision !== runnerSaveRequestId)
          return { ok: true as const, value: undefined }
        const result = await persistSettings({
          defaultRunner: resolution.path,
          richPresenceEnabled: get().richPresenceEnabled,
        })
        return result
      }
      const queued = settingsSaveTail.then(save, save)
      settingsSaveTail = queued.catch(() => undefined)
      const result = await queued
      if (runnerRevision !== runnerSaveRequestId || requestId !== runnersLoadId)
        return
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
  },

  loadDepsStatus: async (runner: string, server = null) => {
    const requestId = ++depsRequestId
    const key = runtimeStatusKey(server, runner)
    set({
      advancedStatus: null,
      advancedStatusKey: key,
      advancedStatusError: null,
    })
    const result = await runSafely(() =>
      api.checkDependencies(server, runner || null),
    )
    if (requestId !== depsRequestId) return
    set({
      advancedStatus: result.ok ? advancedStatusFromDeps(result.value) : null,
      advancedStatusKey: key,
      advancedStatusError: result.ok ? null : result.error,
    })
  },

  applyDepsStatus: (status, key) => {
    ++depsRequestId
    set({
      advancedStatus: advancedStatusFromDeps(status),
      advancedStatusKey: key,
      advancedStatusError: null,
    })
  },

  setRunner: async (path) => {
    if (!get().savingRunner) lastPersistedRunner = get().selectedRunner
    const requestId = ++runnerSaveRequestId
    set({ savingRunner: true, error: null })

    const save = async () => {
      const result = await persistSettings({
        defaultRunner: path,
        richPresenceEnabled: get().richPresenceEnabled,
      })
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

  importRuntimeArchive: async (archivePath) => {
    if (get().importingRuntime) return false
    const runnerRevision = runnerSaveRequestId
    set({ importingRuntime: true, error: null })
    try {
      const result = await runSafely(async () => {
        const path = await api.importManagedRuntimeArchive(archivePath)
        await get().loadRunners()
        const imported = get().runners.find(
          (runner) => runner.id === MANAGED_RUNTIME_ID && runner.path === path,
        )
        if (!imported)
          throw new Error(
            'El runtime importado no aparece como paquete verificado',
          )
        // Import-and-use is explicit, but must not undo a newer user selection.
        if (runnerRevision === runnerSaveRequestId)
          await get().setRunner(imported.path)
      })
      if (!result.ok && runnerRevision === runnerSaveRequestId)
        set({ error: result.error })
      return result.ok && get().error === null
    } finally {
      set({ importingRuntime: false })
    }
  },

  setRichPresenceEnabled: async (enabled) => {
    if (!get().savingPresence) lastPersistedPresence = get().richPresenceEnabled
    const requestId = ++presenceSaveRequestId
    set({ richPresenceEnabled: enabled, savingPresence: true, error: null })

    const save = async () => {
      const result = await persistSettings({
        defaultRunner: get().savingRunner
          ? lastPersistedRunner
          : get().selectedRunner,
        richPresenceEnabled: enabled,
      })
      return result
    }
    const queued = settingsSaveTail.then(save, save)
    settingsSaveTail = queued.catch(() => undefined)
    const result = await queued
    if (requestId !== presenceSaveRequestId) return

    set({
      richPresenceEnabled: result.ok ? enabled : lastPersistedPresence,
      savingPresence: false,
      error: result.ok ? null : result.error,
    })
  },
}))
