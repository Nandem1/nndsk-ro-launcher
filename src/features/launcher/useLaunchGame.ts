import { useEffect, useRef } from 'react'
import { api } from '../../shared/api'
import { launchConfigKey, runtimeStatusKey } from '../../shared/resolveRunner'
import type {
  DependencyStatus,
  LaunchValues,
  ServerConfig,
} from '../../shared/types'
import { useSettingsStore } from '../settings/settings.store'
import { useServersStore } from '../servers/servers.store'
import { clearSettledLaunchFeedback, useLauncherStore } from './launcher.store'
import { useLauncherTask } from './useLauncherTask'
import { refreshGameClients } from './refreshGameClients'

let fallbackClientSequence = 0

type EnvironmentPreparation = 'ready' | 'prepared' | false

function createClientId(): string {
  if (typeof globalThis.crypto?.randomUUID === 'function') {
    return globalThis.crypto.randomUUID()
  }
  fallbackClientSequence += 1
  return `client-${Date.now()}-${fallbackClientSequence}`
}

export function useLaunchGame(server: ServerConfig | null) {
  const preparePromiseRef = useRef<Promise<EnvironmentPreparation> | null>(null)
  const launchInFlightRef = useRef(false)
  const mounted = useRef(true)
  const previousServerId = useRef<string | null | undefined>(undefined)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])
  useEffect(() => {
    const currentId = server?.id ?? null
    const previousId = previousServerId.current
    previousServerId.current = currentId
    if (previousId === undefined || previousId === currentId) return
    clearSettledLaunchFeedback()
  }, [server?.id])
  const selectedRunner = useSettingsStore((s) => s.selectedRunner)
  const {
    status,
    setupProgress,
    error,
    setStatus,
    setProgress,
    addGameLog,
    runTask,
    isBusy,
  } = useLauncherTask()
  const upsertClient = useLauncherStore((s) => s.upsertClient)
  const removeClient = useLauncherStore((s) => s.removeClient)

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
      mounted.current &&
      !!current &&
      launchConfigKey(current, currentRunner) === launchSnapshotKey
    )
  }

  const applyCurrentStatus = (deps: DependencyStatus) => {
    if (isCurrentServer()) {
      useSettingsStore.getState().applyDepsStatus(deps, runtimeSnapshotKey)
    }
  }

  const prepareEnvironment = async (): Promise<EnvironmentPreparation> => {
    if (!server) return false
    if (useSettingsStore.getState().savingRunner) {
      throw new Error(
        'Espera a que termine de guardarse el runner seleccionado',
      )
    }
    let prepared = false
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
          deps.runnerWarning ??
            deps.prefixWarning ??
            'El entorno no está listo y no puede repararse automáticamente',
        )
      }

      setStatus('setting-up')
      addGameLog(
        `Configurando entorno ${deps.prefixScope} en ${deps.prefixPath}...`,
      )
      await api.setupPrefix(
        server,
        selectedRunner || null,
        useLauncherStore.getState().operationId,
      )
      prepared = true
      if (!isCurrentServer()) {
        throw new Error(
          'La configuración cambió mientras se preparaba el entorno; vuelve a comprobarla',
        )
      }
      setProgress(null)

      deps = await api.checkDependencies(server, selectedRunner || null)
      if (!isCurrentServer()) {
        throw new Error('La configuración cambió durante la comprobación final')
      }
      applyCurrentStatus(deps)
      if (!deps.readyToLaunch) {
        throw new Error(
          deps.runnerWarning ??
            deps.prefixWarning ??
            'El entorno siguió incompleto después de configurarlo',
        )
      }
    }

    if (prepared) {
      addGameLog(
        'Entorno listo. Antes de jugar, abre el patcher si hay actualizaciones y configura OpenSetup (resolución y gráficos). También puedes configurar dgVoodoo.',
      )
    }
    return prepared ? 'prepared' : 'ready'
  }

  const handlePrepareEnvironment = (): Promise<EnvironmentPreparation> => {
    if (preparePromiseRef.current) return preparePromiseRef.current
    const promise = runTask(prepareEnvironment)
      .then((result) =>
        result.ok && isCurrentServer() ? result.value : (false as const),
      )
      .finally(() => {
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
    launchInFlightRef.current = true
    try {
      await runTask(async () => {
        if (useSettingsStore.getState().savingRunner) {
          throw new Error(
            'Espera a que termine de guardarse el runner seleccionado',
          )
        }
        if (!environmentPrepared && (await prepareEnvironment()) !== 'ready')
          return
        if (!isCurrentServer()) {
          throw new Error(
            'La configuración del servidor cambió; vuelve a preparar el entorno',
          )
        }

        const clientId = createClientId()
        upsertClient({
          clientId,
          serverId: server.id,
          serverName: server.name,
          status: 'launching',
          pid: null,
        })
        try {
          setStatus('launching')
          addGameLog(`Lanzando ${server.name}...`)

          const client = await api.launchGame(
            clientId,
            server,
            launchValues,
            selectedRunner || null,
          )
          upsertClient(client)
        } catch (cause) {
          removeClient(clientId)
          throw cause
        }
        try {
          await refreshGameClients()
        } catch {
          addGameLog('No se pudo sincronizar la lista de clientes activos')
        }
      })
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
