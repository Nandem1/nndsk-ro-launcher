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
import { useLauncherTask } from './useLauncherTask'

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

      await runTask(async () => {
        setStatus('launching')
        addGameLog(`Lanzando ${server.name}...`)

        await api.launchGame(server, launchValues, selectedRunner || null)
        setStatus('running')
      })
    } finally {
      launchInFlightRef.current = false
    }
  }

  const handleStop = () => {
    void api.stopGame()
  }

  return {
    status,
    setupProgress,
    error,
    isBusy,
    handleLaunch,
    handlePrepareEnvironment,
    handleStop,
  }
}
