import { api } from '../../shared/api'
import type {
  AutobuffConfig,
  AutobuffStatusEvent,
  ServerConfig,
} from '../../shared/types'
import { LAUNCHER_EVENTS } from '../../shared/constants'
import { useServerRuntimeTool } from '../../shared/hooks/useServerRuntimeTool'
import { useLauncherStore } from '../launcher/launcher.store'
import { useLogsStore } from '../logs/logs.store'
import { useSettingsStore } from '../settings/settings.store'
import { useServersStore } from '../servers/servers.store'
import {
  mergeAutobuffConfig,
  type PersistedAutobuffPatch,
  withAutobuffPatch,
} from './autobuff.logic'
import { useAutobuffStore } from './autobuff.store'

export function useAutobuff(server: ServerConfig | null) {
  const status = useAutobuffStore((s) => s.status)
  const busy = useAutobuffStore((s) => s.busy)
  const userEnabled = useAutobuffStore((s) => s.userEnabled)
  const setStatus = useAutobuffStore((s) => s.setStatus)
  const setBusy = useAutobuffStore((s) => s.setBusy)
  const setUserEnabled = useAutobuffStore((s) => s.setUserEnabled)
  const reset = useAutobuffStore((s) => s.reset)
  const addToolLog = useLogsStore((s) => s.addToolLog)

  return useServerRuntimeTool<
    AutobuffConfig,
    AutobuffStatusEvent,
    PersistedAutobuffPatch
  >({
    server,
    isRunning: useLauncherStore((s) => s.status) === 'running',
    selectedRunner: useSettingsStore((s) => s.selectedRunner),
    state: {
      status,
      busy,
      userEnabled,
      setStatus,
      setBusy,
      setUserEnabled,
      reset,
    },
    persistence: {
      persistedConfig: server?.autobuff,
      mergeConfig: mergeAutobuffConfig,
      withPatch: withAutobuffPatch,
      readServerConfig: (currentServer) => currentServer.autobuff,
      persistServer: (serverId, update) =>
        useServersStore.getState().updateServer(serverId, update),
      buildServerConfig: (base, autobuff) => ({ ...base, autobuff }),
    },
    runtime: {
      eventName: LAUNCHER_EVENTS.AUTOBUFF_STATUS,
      toolName: 'AutoBuff',
      addToolLog,
      start: api.startAutobuff,
      stop: api.stopAutobuff,
      updateConfig: api.updateAutobuffConfig,
      isActive: () => useAutobuffStore.getState().status.active,
      statusError: (nextStatus) => nextStatus.error,
    },
  })
}
