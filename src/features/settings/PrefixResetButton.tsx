import { api } from '../../shared/api'
import { Button } from '../../shared/ui/Button'
import { runtimeStatusKey } from '../../shared/resolveRunner'
import { useLauncherTask } from '../launcher/useLauncherTask'
import { useSelectedServer } from '../servers/useSelectedServer'
import { useServersStore } from '../servers/servers.store'
import { useSettingsStore } from './settings.store'
import { useCurrentAdvancedStatus } from './useSelectedRuntimeStatus'

export function PrefixResetButton() {
  const { setStatus, setProgress, setError, addGameLog, runTask, isBusy } =
    useLauncherTask()
  const selectedRunner = useSettingsStore((s) => s.selectedRunner)
  const savingRunner = useSettingsStore((s) => s.savingRunner)
  const advancedStatus = useCurrentAdvancedStatus()
  const loadDepsStatus = useSettingsStore((s) => s.loadDepsStatus)
  const server = useSelectedServer()
  const canReset = advancedStatus?.canReset ?? false
  const canSetup = advancedStatus?.canSetup ?? false
  const prefixPath = advancedStatus?.prefixPath ?? 'el entorno seleccionado'

  const handleReset = async () => {
    if (!server || savingRunner || !advancedStatus || (!canReset && !canSetup))
      return
    const expectedStatusKey = runtimeStatusKey(server, selectedRunner)
    const confirmed = window.confirm(
      canReset
        ? `¿Rearmar el entorno de ${server?.name ?? 'launcher'}?\n\nSe eliminará únicamente ${prefixPath} y se reconstruirá con el runtime Ragnarok administrado.`
        : `¿Reparar el entorno externo de ${server?.name ?? 'launcher'}?\n\nNo se eliminará ${prefixPath}; sólo se instalarán o actualizarán sus componentes.`,
    )
    if (!confirmed) return

    const currentServers = useServersStore.getState()
    const currentServer = currentServers.servers.find(
      (candidate) => candidate.id === currentServers.selectedId,
    )
    const currentSettings = useSettingsStore.getState()
    if (
      !currentServer ||
      currentSettings.savingRunner ||
      !currentSettings.advancedStatus ||
      currentSettings.advancedStatusKey !== expectedStatusKey ||
      currentSettings.advancedStatus.canReset !== canReset ||
      currentSettings.advancedStatus.canSetup !== canSetup ||
      runtimeStatusKey(currentServer, currentSettings.selectedRunner) !==
        expectedStatusKey
    ) {
      return
    }
    const serverSnapshot = currentServer
    const runnerSnapshot = currentSettings.selectedRunner

    setError(null)
    setStatus('setting-up')
    addGameLog(
      canReset ? 'Rearmando entorno...' : 'Reparando entorno externo...',
    )

    await runTask(async () => {
      await api.stopGame()
      if (canReset) {
        await api.resetPrefix(serverSnapshot, runnerSnapshot || null)
      } else {
        await api.setupPrefix(serverSnapshot, runnerSnapshot || null)
      }
      setProgress(null)
      setStatus('idle')
      addGameLog(
        canReset
          ? 'Entorno rearmado correctamente.'
          : 'Entorno externo reparado correctamente.',
      )
      if (
        runnerSnapshot &&
        useServersStore
          .getState()
          .servers.some(
            (candidate) =>
              candidate.id === useServersStore.getState().selectedId &&
              runtimeStatusKey(
                candidate,
                useSettingsStore.getState().selectedRunner,
              ) === expectedStatusKey,
          )
      ) {
        await loadDepsStatus(runnerSnapshot, serverSnapshot)
      }
    }, 'Error al rearmar prefix')
  }

  return (
    <Button
      variant="secondary"
      size="sm"
      block
      onClick={handleReset}
      disabled={
        isBusy ||
        savingRunner ||
        !server ||
        !advancedStatus ||
        (!canReset && !canSetup)
      }
    >
      {!advancedStatus
        ? 'Comprobando entorno...'
        : canReset
          ? 'Rearmar entorno'
          : canSetup
            ? 'Reparar entorno'
            : 'Entorno no reparable'}
    </Button>
  )
}
