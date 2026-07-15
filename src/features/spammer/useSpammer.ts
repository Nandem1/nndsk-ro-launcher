import { api } from '../../shared/api'
import type {
  ServerConfig,
  SpammerConfig,
  SpammerStatusEvent,
} from '../../shared/types'
import { useLauncherStore } from '../launcher/launcher.store'
import { useLogsStore } from '../logs/logs.store'
import { useSettingsStore } from '../settings/settings.store'
import { useServersStore } from '../servers/servers.store'
import {
  mergeSpammerConfig,
  type PersistedSpammerPatch,
  withSpammerPatch,
} from './spammer.logic'
import { useSpammerStore } from './spammer.store'
import { LAUNCHER_EVENTS } from '../../shared/constants'
import { useServerRuntimeTool } from '../../shared/hooks/useServerRuntimeTool'

export function useSpammer(server: ServerConfig | null) {
  const launcherStatus = useLauncherStore((s) => s.status)
  const selectedRunner = useSettingsStore((s) => s.selectedRunner)
  const updateServer = useServersStore((s) => s.updateServer)
  const addToolLog = useLogsStore((s) => s.addToolLog)
  const status = useSpammerStore((s) => s.status)
  const busy = useSpammerStore((s) => s.busy)
  const userEnabled = useSpammerStore((s) => s.userEnabled)
  const setStatus = useSpammerStore((s) => s.setStatus)
  const setBusy = useSpammerStore((s) => s.setBusy)
  const setUserEnabled = useSpammerStore((s) => s.setUserEnabled)
  const reset = useSpammerStore((s) => s.reset)
  const isRunning = launcherStatus === 'running'

  return useServerRuntimeTool<
    SpammerConfig,
    SpammerStatusEvent,
    PersistedSpammerPatch
  >({
    server,
    isRunning,
    selectedRunner,
    eventName: LAUNCHER_EVENTS.SPAMMER_STATUS,
    toolName: 'Spammer',
    persistedConfig: server?.spammer,
    status,
    busy,
    userEnabled,
    setStatus,
    setBusy,
    setUserEnabled,
    reset,
    addToolLog,
    mergeConfig: mergeSpammerConfig,
    withPatch: withSpammerPatch,
    readServerConfig: (currentServer) => currentServer.spammer,
    persistServer: updateServer,
    startTool: api.startSpammer,
    stopTool: api.stopSpammer,
    updateToolConfig: api.updateSpammerConfig,
    buildServerConfig: (baseServer, spammer) => ({ ...baseServer, spammer }),
    isRuntimeActive: () => useSpammerStore.getState().status.armed,
    statusError: (nextStatus) => nextStatus.error,
  })
}
