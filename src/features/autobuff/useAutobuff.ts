import { api } from '../../shared/api'
import type { AutobuffConfig, AutobuffStatusEvent, ServerConfig } from '../../shared/types'
import { LAUNCHER_EVENTS } from '../../shared/constants'
import { useServerRuntimeTool } from '../../shared/hooks/useServerRuntimeTool'
import { useLauncherStore } from '../launcher/launcher.store'
import { useLogsStore } from '../logs/logs.store'
import { useSettingsStore } from '../settings/settings.store'
import { useServersStore } from '../servers/servers.store'
import { mergeAutobuffConfig, type PersistedAutobuffPatch, withAutobuffPatch } from './autobuff.logic'
import { useAutobuffStore } from './autobuff.store'

export function useAutobuff(server: ServerConfig | null) {
  return useServerRuntimeTool<AutobuffConfig, AutobuffStatusEvent, PersistedAutobuffPatch>({
    server, isRunning: useLauncherStore((s) => s.status) === 'running', selectedRunner: useSettingsStore((s) => s.selectedRunner), eventName: LAUNCHER_EVENTS.AUTOBUFF_STATUS, toolName: 'AutoBuff', persistedConfig: server?.autobuff,
    status: useAutobuffStore((s) => s.status), busy: useAutobuffStore((s) => s.busy), userEnabled: useAutobuffStore((s) => s.userEnabled), setStatus: useAutobuffStore((s) => s.setStatus), setBusy: useAutobuffStore((s) => s.setBusy), setUserEnabled: useAutobuffStore((s) => s.setUserEnabled), reset: useAutobuffStore((s) => s.reset), addToolLog: useLogsStore((s) => s.addToolLog),
    mergeConfig: mergeAutobuffConfig, withPatch: withAutobuffPatch, persistConfig: (serverId, autobuff) => useServersStore.getState().updateServer(serverId, { autobuff }), startTool: api.startAutobuff, stopTool: api.stopAutobuff, updateToolConfig: api.updateAutobuffConfig, buildServerConfig: (base, autobuff) => ({ ...base, autobuff }), isRuntimeActive: () => useAutobuffStore.getState().status.active, statusError: (status) => status.error,
  })
}
