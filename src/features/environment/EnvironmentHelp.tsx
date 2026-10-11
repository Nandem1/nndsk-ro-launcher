import { useEffect, useState } from 'react'
import { runtimeStatusKey } from '../../shared/resolveRunner'
import { useSelectedServer } from '../servers/useSelectedServer'
import { useSettingsStore } from '../settings/settings.store'
import {
  clearSettledLaunchFeedback,
  useLauncherStore,
} from '../launcher/launcher.store'
import {
  useCurrentAdvancedStatus,
  useCurrentRuntimeStatusError,
} from '../settings/useSelectedRuntimeStatus'
import { EnvironmentHelpDialog } from './EnvironmentHelpDialog'
import { useEnvironmentStore } from './environment.store'

export function EnvironmentHelp({
  busy,
  onPrepare,
}: {
  busy: boolean
  onPrepare: () => void
}) {
  const server = useSelectedServer()
  const selectedRunner = useSettingsStore((state) => state.selectedRunner)
  const loadDepsStatus = useSettingsStore((state) => state.loadDepsStatus)
  const status = useCurrentAdvancedStatus()
  const clientsActive = useLauncherStore((state) => state.clients.length > 0)
  const error = useCurrentRuntimeStatusError()
  const helpContext = useEnvironmentStore((state) => state.helpContext)
  const closeHelp = useEnvironmentStore((state) => state.closeHelp)
  const [checking, setChecking] = useState(false)
  const context = runtimeStatusKey(server, selectedRunner)

  useEffect(() => {
    if (helpContext && helpContext !== context) closeHelp()
  }, [helpContext, context, closeHelp])

  if (!server || helpContext !== context) return null

  const recheck = async () => {
    if (checking || busy) return
    setChecking(true)
    try {
      await loadDepsStatus(selectedRunner, server)
      const current = useSettingsStore.getState()
      const assessment = current.advancedStatus?.environment
      if (
        current.advancedStatusKey === context &&
        !current.advancedStatusError &&
        assessment?.schemaVersion === 1 &&
        assessment.launch !== 'blocked'
      ) {
        clearSettledLaunchFeedback()
      }
    } finally {
      setChecking(false)
    }
  }

  return (
    <EnvironmentHelpDialog
      key={context}
      serverName={server.name}
      assessment={
        status?.environment?.schemaVersion === 1 ? status.environment : null
      }
      checking={checking || (!status && !error)}
      busy={busy}
      clientsActive={clientsActive}
      error={error}
      onClose={closeHelp}
      onRecheck={() => void recheck()}
      onPrepare={onPrepare}
    />
  )
}
