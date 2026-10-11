import { api } from '../../shared/api'
import { Button } from '../../shared/ui/Button'
import { runtimeStatusKey } from '../../shared/resolveRunner'
import { useLauncherTask } from '../launcher/useLauncherTask'
import { useLauncherStore } from '../launcher/launcher.store'
import { useSelectedServer } from '../servers/useSelectedServer'
import { useServersStore } from '../servers/servers.store'
import { useSettingsStore } from './settings.store'
import { useCurrentAdvancedStatus } from './useSelectedRuntimeStatus'
import { useEnvironmentStore } from '../environment/environment.store'

export function PrefixResetButton() {
  const { setStatus, setProgress, addGameLog, runTask, isBusy } =
    useLauncherTask()
  const selectedRunner = useSettingsStore((s) => s.selectedRunner)
  const savingRunner = useSettingsStore((s) => s.savingRunner)
  const advancedStatus = useCurrentAdvancedStatus()
  const loadDepsStatus = useSettingsStore((s) => s.loadDepsStatus)
  const server = useSelectedServer()
  const openHelp = useEnvironmentStore((state) => state.openHelp)
  const activeClients = useLauncherStore((s) => s.clients.length)
  const canReset = advancedStatus?.canReset ?? false
  const canSetup = advancedStatus?.canSetup ?? false
  const needsHelp = !!advancedStatus?.environment && !canReset && !canSetup
  const prefixPath = advancedStatus?.prefixPath ?? 'el entorno seleccionado'

  const handleReset = async () => {
    if (
      !server ||
      savingRunner ||
      activeClients > 0 ||
      !advancedStatus ||
      (!canReset && !canSetup)
    )
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

    await runTask(async () => {
      const operationId = useLauncherStore.getState().operationId
      setStatus('setting-up')
      addGameLog(
        canReset ? 'Rearmando entorno...' : 'Reparando entorno externo...',
      )
      if (canReset) {
        await api.resetPrefix(
          serverSnapshot,
          runnerSnapshot || null,
          operationId,
        )
      } else {
        await api.setupPrefix(
          serverSnapshot,
          runnerSnapshot || null,
          operationId,
        )
      }
      setProgress(null)
      addGameLog(
        canReset
          ? 'Entorno rearmado correctamente.'
          : 'Entorno externo reparado correctamente.',
      )
      if (
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
      onClick={() => {
        if (needsHelp && server)
          openHelp(runtimeStatusKey(server, selectedRunner))
        else void handleReset()
      }}
      disabled={
        isBusy ||
        activeClients > 0 ||
        savingRunner ||
        !server ||
        !advancedStatus ||
        (!canReset && !canSetup && !needsHelp)
      }
    >
      {activeClients > 0
        ? 'Cierra los clientes para rearmar'
        : !advancedStatus
          ? 'Comprobando entorno...'
          : canReset
            ? 'Rearmar entorno'
            : canSetup
              ? 'Reparar entorno'
              : needsHelp
                ? 'Revisar requisitos'
                : 'Entorno no reparable'}
    </Button>
  )
}
