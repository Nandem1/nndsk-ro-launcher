import { api } from '../../shared/api'
import type {
  AutopotConfig,
  AutopotStatusEvent,
  ServerConfig,
} from '../../shared/types'
import { useLauncherStore } from '../launcher/launcher.store'
import { useLogsStore } from '../logs/logs.store'
import { useSettingsStore } from '../settings/settings.store'
import { useServersStore } from '../servers/servers.store'
import {
  mergeAutopotConfig,
  type PersistedAutopotPatch,
  withAutopotPatch,
} from './autopot.logic'
import { useAutopotStore } from './autopot.store'
import { LAUNCHER_EVENTS } from '../../shared/constants'
import { useServerRuntimeTool } from '../../shared/hooks/useServerRuntimeTool'

export function useAutopot(server: ServerConfig | null) {
  const launcherStatus = useLauncherStore((s) => s.status)
  const selectedRunner = useSettingsStore((s) => s.selectedRunner)
  const updateServer = useServersStore((s) => s.updateServer)
  const addToolLog = useLogsStore((s) => s.addToolLog)
  const status = useAutopotStore((s) => s.status)
  const busy = useAutopotStore((s) => s.busy)
  const userEnabled = useAutopotStore((s) => s.userEnabled)
  const setStatus = useAutopotStore((s) => s.setStatus)
  const setBusy = useAutopotStore((s) => s.setBusy)
  const setUserEnabled = useAutopotStore((s) => s.setUserEnabled)
  const reset = useAutopotStore((s) => s.reset)
  const isRunning = launcherStatus === 'running'

  return useServerRuntimeTool<
    AutopotConfig,
    AutopotStatusEvent,
    PersistedAutopotPatch
  >({
    server,
    isRunning,
    selectedRunner,
    eventName: LAUNCHER_EVENTS.AUTOPOT_STATUS,
    toolName: 'AutoPot',
    persistedConfig: server?.autopot,
    status,
    busy,
    userEnabled,
    setStatus,
    setBusy,
    setUserEnabled,
    reset,
    addToolLog,
    mergeConfig: mergeAutopotConfig,
    withPatch: withAutopotPatch,
    readServerConfig: (currentServer) => currentServer.autopot,
    persistServer: updateServer,
    startTool: api.startAutopot,
    stopTool: api.stopAutopot,
    updateToolConfig: api.updateAutopotConfig,
    buildServerConfig: (baseServer, autopot) => ({ ...baseServer, autopot }),
    isRuntimeActive: () => useAutopotStore.getState().status.active,
    statusError: (nextStatus) => nextStatus.error,
  })
}
