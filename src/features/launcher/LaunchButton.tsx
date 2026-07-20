import { useEffect, useState } from 'react'
import { Hammer, Play, RotateCcw } from 'lucide-react'
import { requiredLaunchFields } from '../../shared/contracts'
import { launchConfigKey } from '../../shared/resolveRunner'
import { Button, type ButtonVariant } from '../../shared/ui/Button'
import { useLaunchGame } from './useLaunchGame'
import { useSelectedServer } from '../servers/useSelectedServer'
import { useSettingsStore } from '../settings/settings.store'
import { useCurrentAdvancedStatus } from '../settings/useSelectedRuntimeStatus'
import { LaunchFieldsModal } from './LaunchFieldsModal'

export function LaunchButton() {
  const server = useSelectedServer()
  const [showLaunchFields, setShowLaunchFields] = useState(false)
  const savingRunner = useSettingsStore((s) => s.savingRunner)
  const selectedRunner = useSettingsStore((s) => s.selectedRunner)
  const advancedStatus = useCurrentAdvancedStatus()
  const {
    status,
    setupProgress,
    error,
    isBusy,
    handleLaunch,
    handlePrepareEnvironment,
    handleStop,
  } = useLaunchGame(server)

  const serverLaunchKey = server ? launchConfigKey(server, selectedRunner) : ''
  useEffect(() => setShowLaunchFields(false), [serverLaunchKey])

  const isDisabled = !server || isBusy || savingRunner
  const buildMode = status === 'idle' && !advancedStatus?.readyToLaunch

  const labels: Record<typeof status, string> = {
    idle: buildMode
      ? advancedStatus?.canSetup === false
        ? 'Revisar entorno'
        : 'Preparar entorno'
      : 'Jugar',
    checking: 'Comprobando...',
    'setting-up': 'Configurando...',
    launching: 'Iniciando...',
    running: 'Jugando...',
    error: 'Reintentar',
  }

  const variant: ButtonVariant =
    status === 'running'
      ? 'success'
      : status === 'error'
        ? 'danger'
        : buildMode
          ? 'secondary'
          : 'primary'

  const icon =
    status === 'error' ? (
      <RotateCcw className="w-4 h-4" />
    ) : buildMode ? (
      <Hammer className="w-4 h-4" />
    ) : status === 'idle' ? (
      <Play className="w-4 h-4" />
    ) : null

  return (
    <div className="flex flex-col gap-2 shrink-0 border-t border-white/[0.06] pt-3">
      {status === 'setting-up' && setupProgress && (
        <div className="space-y-1">
          <div className="flex justify-between gap-2 text-[10px] text-zinc-500">
            <span className="truncate">{setupProgress.step}</span>
            <span className="shrink-0 tabular-nums">
              {setupProgress.percent}%
            </span>
          </div>
          <div className="w-full bg-zinc-800 rounded-full h-1.5 overflow-hidden">
            <div
              className="h-full bg-gradient-to-r from-amber-600 via-amber-300 to-amber-400 rounded-full transition-all duration-500"
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
            if (await handlePrepareEnvironment()) {
              setShowLaunchFields(true)
            }
          })()
        }}
        disabled={isDisabled}
        className={status === 'running' ? 'cursor-default' : ''}
      >
        {icon}
        {labels[status]}
      </Button>
      {(status === 'running' || status === 'launching') && (
        <button
          type="button"
          onClick={handleStop}
          className="w-full py-2 rounded-xl text-xs text-zinc-500 hover:text-red-400 border border-zinc-800/80
            hover:border-red-500/30 hover:bg-red-500/5 transition-colors"
        >
          Detener juego
        </button>
      )}
      {status === 'error' && error && (
        <p className="text-red-400 text-[11px] text-center px-2 leading-relaxed">
          {error}
        </p>
      )}
      {showLaunchFields && server && (
        <LaunchFieldsModal
          serverName={server.name}
          fields={requiredLaunchFields(server.launch)}
          onCancel={() => setShowLaunchFields(false)}
          onSubmit={(values) => {
            setShowLaunchFields(false)
            void handleLaunch(values, true)
          }}
        />
      )}
    </div>
  )
}
