import { useEffect, useState } from 'react'
import { Hammer, Play, RotateCcw } from 'lucide-react'
import { requiredLaunchFields } from '../../shared/contracts'
import { launchConfigKey } from '../../shared/resolveRunner'
import { Button, type ButtonVariant } from '../../shared/ui/Button'
import { useLaunchGame } from './useLaunchGame'
import { useSelectedServer } from '../servers/useSelectedServer'
import { useSettingsStore } from '../settings/settings.store'
import {
  useCurrentAdvancedStatus,
  useCurrentRuntimeStatusError,
} from '../settings/useSelectedRuntimeStatus'
import { LaunchFieldsModal } from './LaunchFieldsModal'
import { useLauncherStore } from './launcher.store'

export function LaunchButton() {
  const server = useSelectedServer()
  const [fieldsKey, setFieldsKey] = useState<string | null>(null)
  const savingRunner = useSettingsStore((s) => s.savingRunner)
  const selectedRunner = useSettingsStore((s) => s.selectedRunner)
  const advancedStatus = useCurrentAdvancedStatus()
  const depsError = useCurrentRuntimeStatusError()
  const {
    status,
    setupProgress,
    error,
    isBusy,
    handleLaunch,
    handlePrepareEnvironment,
  } = useLaunchGame(server)
  const existingClients = useLauncherStore(
    (state) =>
      state.clients.filter((client) => client.serverId === server?.id).length,
  )

  const serverLaunchKey = server ? launchConfigKey(server, selectedRunner) : ''
  useEffect(() => setFieldsKey(null), [serverLaunchKey])

  const checkingEnvironment = !advancedStatus && !depsError
  const isDisabled = !server || isBusy || savingRunner || checkingEnvironment
  const buildMode =
    status === 'idle' && !!advancedStatus && !advancedStatus.readyToLaunch

  const labels: Record<typeof status, string> = {
    idle: checkingEnvironment
      ? 'Comprobando entorno...'
      : depsError
        ? 'Comprobar entorno'
        : buildMode
          ? advancedStatus?.canSetup === false
            ? 'Revisar entorno'
            : 'Preparar entorno'
          : existingClients > 0
            ? 'Abrir otro cliente'
            : 'Jugar',
    checking: 'Comprobando...',
    'setting-up': 'Configurando...',
    launching: 'Iniciando...',
    error: 'Reintentar',
  }

  const variant: ButtonVariant =
    status === 'error' ? 'danger' : buildMode ? 'secondary' : 'primary'

  const icon =
    status === 'error' ? (
      <RotateCcw className="w-4 h-4" />
    ) : buildMode ? (
      <Hammer className="w-4 h-4" />
    ) : status === 'idle' ? (
      <Play className="w-4 h-4" />
    ) : null

  return (
    <div className="flex flex-col gap-2 shrink-0 border-t border-line pt-3">
      {status === 'setting-up' && setupProgress && (
        <div className="space-y-1">
          <div className="flex justify-between gap-2 text-caption text-muted">
            <span className="truncate">{setupProgress.step}</span>
            <span className="font-mono shrink-0 tabular-nums">
              {setupProgress.percent}%
            </span>
          </div>
          <div className="w-full bg-panel-raised rounded-pill h-1.5 overflow-hidden">
            <div
              className="h-full bg-muted rounded-pill transition-colors duration-150"
              style={{ width: `${setupProgress.percent}%` }}
            />
          </div>
        </div>
      )}
      <Button
        variant={variant}
        size="lg"
        block
        onClick={() => {
          void (async () => {
            if (!advancedStatus?.readyToLaunch) {
              await handlePrepareEnvironment()
              return
            }
            let fields: string[]
            try {
              fields = requiredLaunchFields(server?.launch)
            } catch {
              await handleLaunch()
              return
            }
            if (!fields.length) {
              await handleLaunch()
              return
            }
            if ((await handlePrepareEnvironment()) === 'ready') {
              setFieldsKey(serverLaunchKey)
            }
          })()
        }}
        disabled={isDisabled}
      >
        {icon}
        {labels[status]}
      </Button>
      {depsError && status !== 'error' && (
        <p className="text-bad text-detail text-center px-2 leading-relaxed">
          {depsError}
        </p>
      )}
      {status === 'error' && error && (
        <p className="text-bad text-detail text-center px-2 leading-relaxed">
          {error}
        </p>
      )}
      {fieldsKey === serverLaunchKey && server && (
        <LaunchFieldsModal
          serverName={server.name}
          fields={requiredLaunchFields(server.launch)}
          onCancel={() => setFieldsKey(null)}
          onSubmit={(values) => {
            setFieldsKey(null)
            void handleLaunch(values, true)
          }}
        />
      )}
    </div>
  )
}
