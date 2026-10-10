import { useState } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { useSettingsStore } from './settings.store'
import { Panel } from '../../shared/ui/Panel'
import { DarkSelect } from '../../shared/ui/DarkSelect'
import { Button } from '../../shared/ui/Button'
import { DataText } from '../../shared/ui/DataText'
import { runSafely } from '../../shared/async'
import { isLauncherBusy, useLauncherStore } from '../launcher/launcher.store'
import { useSelectedServer } from '../servers/useSelectedServer'
import { runnerSelectionOptions } from '../../shared/resolveRunner'

export function RunnerSelector() {
  const {
    runners,
    selectedRunner,
    savingRunner,
    importingRuntime,
    error,
    setRunner,
    importRuntimeArchive,
  } = useSettingsStore()
  const [pickingArchive, setPickingArchive] = useState(false)
  const [pickerError, setPickerError] = useState<string | null>(null)
  const launcherStatus = useLauncherStore((state) => state.status)
  const activeClients = useLauncherStore((state) => state.clients.length)
  const server = useSelectedServer()
  const launcherBusy = isLauncherBusy(launcherStatus) || activeClients > 0
  const serverRunner = server?.runner?.trim() ?? ''
  const serverRunnerName =
    runners.find((runner) => runner.path === serverRunner)?.name ??
    `Selección anterior conservada · ${serverRunner}`

  const detected = runners.some((runner) => runner.path === selectedRunner)
  const options = runnerSelectionOptions(runners, selectedRunner)

  const importBusy = pickingArchive || importingRuntime

  const importRuntime = async () => {
    setPickingArchive(true)
    setPickerError(null)
    try {
      const result = await runSafely(() =>
        open({
          title: 'Importar nndsk-ro-proton verificado',
          multiple: false,
          directory: false,
          filters: [{ name: 'Runtime tar.zst', extensions: ['zst'] }],
        }),
      )
      if (!result.ok) {
        setPickerError(result.error)
        return
      }
      if (typeof result.value !== 'string') return
      const launcher = useLauncherStore.getState()
      if (isLauncherBusy(launcher.status) || launcher.clients.length > 0) {
        setPickerError(
          'Espera a que termine la sesión antes de importar el runtime',
        )
        return
      }
      await importRuntimeArchive(result.value)
    } finally {
      setPickingArchive(false)
    }
  }

  return (
    <Panel title="Runner predeterminado" className="shrink-0">
      {options.length > 0 && (
        <DarkSelect
          value={selectedRunner}
          options={options}
          onChange={setRunner}
          disabled={savingRunner || launcherBusy || importBusy}
        />
      )}
      <div className="mt-2">
        <Button
          size="xs"
          disabled={savingRunner || launcherBusy || importBusy}
          onClick={() => void importRuntime()}
        >
          {importBusy
            ? 'Importando runtime...'
            : 'Importar y usar nndsk-ro-proton'}
        </Button>
        <p className="mt-1 text-micro leading-relaxed text-muted">
          <DataText>
            nndsk-ro-proton y Wine 7.16 Staging/TkG amd64 se descargan y
            verifican por SHA-256 al preparar el entorno. Son los dos runners
            ofrecidos. La importación local es opcional y cambia sólo el
            predeterminado global, no los runners propios de cada servidor.
          </DataText>
        </p>
      </div>
      {savingRunner && (
        <p className="mt-1.5 text-caption text-muted">Guardando selección...</p>
      )}
      {(error || pickerError) && (
        <p role="alert" className="mt-1.5 text-caption text-bad">
          {pickerError || error}
        </p>
      )}
      {!detected && selectedRunner && (
        <p className="mt-1.5 text-caption leading-relaxed text-muted">
          Tu selección anterior está fuera del catálogo ofrecido y se conserva
          sin migrar su prefix. Para un entorno nuevo, selecciona uno de los dos
          runners administrados.
        </p>
      )}
      {server && serverRunner && (
        <div className="mt-2 border-l-[3px] border-warn pl-3 py-1">
          <p className="text-caption leading-relaxed text-muted">
            Runner efectivo de {server.name}:{' '}
            <span className="text-muted font-mono tabular-nums">
              {serverRunnerName}
            </span>
          </p>
          <p className="mt-0.5 text-micro leading-relaxed text-muted">
            Propio del servidor; el predeterminado global no lo reemplaza.
          </p>
        </div>
      )}
    </Panel>
  )
}
