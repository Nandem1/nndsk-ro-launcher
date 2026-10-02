import { useEffect } from 'react'
import { useSelectedServer } from '../servers/useSelectedServer'
import { useServersStore } from '../servers/servers.store'
import { runtimeStatusKey } from '../../shared/resolveRunner'
import { useSettingsStore } from './settings.store'

export function useSelectedRuntimeStatus() {
  const server = useSelectedServer()
  const selectedRunner = useSettingsStore((state) => state.selectedRunner)
  const loadDepsStatus = useSettingsStore((state) => state.loadDepsStatus)
  const settingsLoading = useSettingsStore((state) => state.loading)
  const serversLoading = useServersStore((state) => state.loading)
  const statusKey = runtimeStatusKey(server, selectedRunner)

  useEffect(() => {
    if (settingsLoading || serversLoading) return
    void loadDepsStatus(selectedRunner, server)
    // `runtimeKey` excluye AutoPot/Spammer/AutoBuff para no reescanear el entorno
    // cuando sólo cambia una herramienta de combate.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loadDepsStatus, statusKey, settingsLoading, serversLoading])
}

export function useCurrentRuntimeStatusError() {
  const server = useSelectedServer()
  const selectedRunner = useSettingsStore((state) => state.selectedRunner)
  const statusKey = useSettingsStore((state) => state.advancedStatusKey)
  const error = useSettingsStore((state) => state.advancedStatusError)

  return statusKey === runtimeStatusKey(server, selectedRunner) ? error : null
}

export function useCurrentAdvancedStatus() {
  const server = useSelectedServer()
  const selectedRunner = useSettingsStore((state) => state.selectedRunner)
  const savingRunner = useSettingsStore((state) => state.savingRunner)
  const status = useSettingsStore((state) => state.advancedStatus)
  const statusKey = useSettingsStore((state) => state.advancedStatusKey)
  const currentKey = runtimeStatusKey(server, selectedRunner)

  return !savingRunner && statusKey === currentKey ? status : null
}
