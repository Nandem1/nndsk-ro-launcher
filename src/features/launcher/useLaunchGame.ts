import { useRef } from 'react'
import { api } from '../../shared/api'
import { launchConfigKey, runtimeStatusKey } from '../../shared/resolveRunner'
import type {
  DependencyStatus,
  LaunchValues,
  ServerConfig,
} from '../../shared/types'
import { useSettingsStore } from '../settings/settings.store'
import { useServersStore } from '../servers/servers.store'
import { useLauncherStore } from './launcher.store'
import { useLauncherTask } from './useLauncherTask'

let fallbackClientSequence = 0

function createClientId(): string {
  if (typeof globalThis.crypto?.randomUUID === 'function') {
    return globalThis.crypto.randomUUID()
  }
  fallbackClientSequence += 1
  return `client-${Date.now()}-${fallbackClientSequence}`
}

export function useLaunchGame(server: ServerConfig | null) {
  const preparePromiseRef = useRef<Promise<boolean> | null>(null)
  const launchInFlightRef = useRef(false)
  const selectedRunner = useSettingsStore((s) => s.selectedRunner)
  const {
    status,
    setupProgress,
    error,
    setStatus,
    setProgress,
    setError,
    addGameLog,
    runTask,
    isBusy,
  } = useLauncherTask()
  const upsertClient = useLauncherStore((s) => s.upsertClient)
  const removeClient = useLauncherStore((s) => s.removeClient)
  const setClients = useLauncherStore((s) => s.setClients)

  const launchSnapshotKey = server
    ? launchConfigKey(server, selectedRunner)
    : null
  const runtimeSnapshotKey = runtimeStatusKey(server, selectedRunner)
  const isCurrentServer = () => {
    if (!server || !launchSnapshotKey) return false
    const state = useServersStore.getState()
    const current = state.servers.find(
      (candidate) => candidate.id === state.selectedId,
    )
    const currentRunner = useSettingsStore.getState().selectedRunner
    return (
      !!current && launchConfigKey(current, currentRunner) === launchSnapshotKey
    )
  }

  const applyCurrentStatus = (deps: DependencyStatus) => {
    if (isCurrentServer()) {
      useSettingsStore.getState().applyDepsStatus(deps, runtimeSnapshotKey)
    }
  }

  const prepareEnvironment = async (): Promise<boolean> => {
    if (!server) return false
    if (useSettingsStore.getState().savingRunner) {
      setError('Espera a que termine de guardarse el runner seleccionado')
      setStatus('error')
      return false
    }
    if (status === 'error') setStatus('idle')
    setError(null)
    setStatus('checking')
    let ready = false
    const result = await runTask(async () => {
      let deps = await api.checkDependencies(server, selectedRunner || null)
      if (!isCurrentServer()) {
        throw new Error(
          'La configuración del servidor o runner cambió durante la comprobación',
        )
      }
      applyCurrentStatus(deps)

      if (deps.audioWarning) addGameLog(deps.audioWarning)

      if (!deps.readyToLaunch) {
        if (!deps.canSetup) {
          throw new Error(
            deps.prefixWarning ??
              'El entorno no está listo y no puede repararse automáticamente',
          )
        }

        setStatus('setting-up')
        addGameLog(
          `Configurando entorno ${deps.prefixScope} en ${deps.prefixPath}...`,
        )
        await api.setupPrefix(server, selectedRunner || null)
        if (!isCurrentServer()) {
          throw new Error(
            'La configuración cambió mientras se preparaba el entorno; vuelve a comprobarla',
          )
        }
        setProgress(null)

        deps = await api.checkDependencies(server, selectedRunner || null)
        if (!isCurrentServer()) {
          throw new Error(
            'La configuración cambió durante la comprobación final',
          )
        }
        applyCurrentStatus(deps)
        if (!deps.readyToLaunch) {
          throw new Error(
            deps.prefixWarning ??
              'El entorno siguió incompleto después de configurarlo',
          )
        }
      }

      ready = true
      setStatus('idle')
    })
    return result.ok && ready && isCurrentServer()
  }

  const handlePrepareEnvironment = (): Promise<boolean> => {
    if (preparePromiseRef.current) return preparePromiseRef.current
    const promise = prepareEnvironment().finally(() => {
      if (preparePromiseRef.current === promise) {
        preparePromiseRef.current = null
      }
    })
    preparePromiseRef.current = promise
    return promise
  }

  const handleLaunch = async (
    launchValues: LaunchValues = {},
    environmentPrepared = false,
  ) => {
    if (!server || launchInFlightRef.current) return
    if (useSettingsStore.getState().savingRunner) {
      setError('Espera a que termine de guardarse el runner seleccionado')
      setStatus('error')
      return
    }
    launchInFlightRef.current = true
    try {
      if (!environmentPrepared && !(await handlePrepareEnvironment())) return
      if (!isCurrentServer()) {
        setError(
          'La configuración del servidor cambió; vuelve a preparar el entorno',
        )
        setStatus('error')
        return
      }

      const clientId = createClientId()
      upsertClient({
        clientId,
        serverId: server.id,
        serverName: server.name,
        status: 'launching',
        pid: null,
      })
      const result = await runTask(async () => {
        setStatus('launching')
        addGameLog(`Lanzando ${server.name}...`)

        const client = await api.launchGame(
          clientId,
          server,
          launchValues,
          selectedRunner || null,
        )
        upsertClient(client)
        setStatus('idle')
      })
      if (!result.ok) {
        removeClient(clientId)
      } else {
        try {
          setClients(await api.listGameClients())
        } catch {
          addGameLog('No se pudo sincronizar la lista de clientes activos')
        }
      }
    } finally {
      launchInFlightRef.current = false
    }
  }

  return {
    status,
    setupProgress,
    error,
    isBusy,
    handleLaunch,
    handlePrepareEnvironment,
  }
}
