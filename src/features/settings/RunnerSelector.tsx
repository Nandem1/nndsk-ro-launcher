import { useSettingsStore } from './settings.store'
import { Panel } from '../../shared/ui/Panel'
import { DarkSelect } from '../../shared/ui/DarkSelect'
import { isLauncherBusy, useLauncherStore } from '../launcher/launcher.store'

export function RunnerSelector() {
  const { runners, selectedRunner, savingRunner, error, setRunner } =
    useSettingsStore()
  const launcherStatus = useLauncherStore((state) => state.status)
  const launcherBusy = isLauncherBusy(launcherStatus)

  const detected = runners.some((runner) => runner.path === selectedRunner)
  const options = runners.map((runner) => ({
    value: runner.path,
    label: runner.name,
  }))
  if (selectedRunner && !detected) {
    options.push({
      value: selectedRunner,
      label: `No detectado · ${selectedRunner}`,
    })
  }

  if (options.length === 0) return null

  return (
    <Panel title="Runner predeterminado" className="shrink-0">
      <DarkSelect
        value={selectedRunner}
        options={options}
        onChange={setRunner}
        disabled={savingRunner || launcherBusy}
      />
      {savingRunner && (
        <p className="mt-1.5 text-[10px] text-zinc-500">
          Guardando selección...
        </p>
      )}
      {error && (
        <p role="alert" className="mt-1.5 text-[10px] text-red-400">
          {error}
        </p>
      )}
      {!detected && selectedRunner && (
        <p className="mt-1.5 text-[10px] leading-relaxed text-amber-400/80">
          La ruta guardada ya no está disponible. Selecciona un runner detectado
          para poder preparar o iniciar clientes.
        </p>
      )}
    </Panel>
  )
}
