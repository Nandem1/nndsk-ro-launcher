import { useSettingsStore } from './settings.store'
import { Panel } from '../../shared/ui/Panel'
import { DarkSelect } from '../../shared/ui/DarkSelect'
import { isLauncherBusy, useLauncherStore } from '../launcher/launcher.store'
import { useSelectedServer } from '../servers/useSelectedServer'

export function RunnerSelector() {
  const { runners, selectedRunner, savingRunner, error, setRunner } =
    useSettingsStore()
  const launcherStatus = useLauncherStore((state) => state.status)
  const activeClients = useLauncherStore((state) => state.clients.length)
  const server = useSelectedServer()
  const launcherBusy = isLauncherBusy(launcherStatus) || activeClients > 0
  const serverRunner = server?.runner?.trim() ?? ''
  const serverRunnerName =
    runners.find((runner) => runner.path === serverRunner)?.name ?? serverRunner

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
      {server && serverRunner && (
        <div className="mt-2 rounded-md border border-amber-500/15 bg-amber-500/5 px-2.5 py-2">
          <p className="text-[10px] leading-relaxed text-amber-300/90">
            Runner efectivo de {server.name}: {serverRunnerName}
          </p>
          <p className="mt-0.5 text-[9px] leading-relaxed text-zinc-500">
            Propio del servidor; el predeterminado global no lo reemplaza.
          </p>
        </div>
      )}
    </Panel>
  )
}
