import { useEffect } from 'react'
import { useLauncherStore } from '../features/launcher/launcher.store'
import { modeForStatus, useUiModeStore } from './uiMode.store'

export function useUiModeTransition() {
  const status = useLauncherStore((s) => s.status)
  const activeClients = useLauncherStore((s) => s.clients.length)

  useEffect(() => {
    const next = modeForStatus(status, activeClients)
    if (next === useUiModeStore.getState().mode) return
    useUiModeStore.getState().setMode(next)
  }, [activeClients, status])
}
