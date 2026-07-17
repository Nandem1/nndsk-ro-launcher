import { Zap } from 'lucide-react'
import { useUiModeStore } from '../../app/uiMode.store'
import { useLauncherStore } from '../launcher/launcher.store'
import { useSelectedServer } from '../servers/useSelectedServer'
import { Panel, resolveToolTone } from '../../shared/ui/Panel'
import { ToggleSwitch } from '../../shared/ui/ToggleSwitch'
import { formatSpammerKeys } from './spammer.logic'
import { GearSwitchEditor } from './GearSwitchEditor'
import { SpammerKeyboard } from './SpammerKeyboard'
import { useSpammer } from './useSpammer'

export function SpammerPanel() {
  const server = useSelectedServer()
  const { config, status, busy, isRunning, error, setEnabled, updateField } =
    useSpammer(server)
  const launching = useLauncherStore((state) => state.status === 'launching')
  const hero = useUiModeStore((state) => state.mode === 'ingame')
  const available = isRunning && !!server
  const minimumDelayMs = 10
  const keysLabel = formatSpammerKeys(config.keys)

  const statusLabel = (() => {
    if (!available || !status.armed) return 'Inactivo'
    if (status.spamming && status.key) {
      return `${status.cycleCount.toLocaleString()} ciclos · ${status.key} + click`
    }
    return `Standby — ${keysLabel}`
  })()

  const statusText = !server
    ? 'Selecciona un servidor'
    : launching
      ? 'Iniciando juego...'
      : !isRunning
        ? 'Inicia el juego'
        : config.keys.length === 0
          ? 'Selecciona al menos una tecla'
          : status.spamming
            ? 'Spameando...'
            : 'Mantén una tecla configurada en el juego'

  const tone = resolveToolTone(
    available,
    config.enabled && status.armed,
    !!error,
    'warning',
  )

  return (
    <Panel
      title="Spammer"
      compact
      hero={hero}
      tone={tone}
      className="h-full"
      leading={<Zap className="w-3 h-3 text-zinc-600 shrink-0" aria-hidden />}
    >
      <div className="flex-1 min-h-0 overflow-y-auto space-y-2 pr-0.5">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <p
              className={`text-sm font-semibold truncate ${
                status.spamming ? 'text-amber-200' : 'text-zinc-100'
              }`}
            >
              {statusLabel}
            </p>
            <p
              className={`text-[10px] ${launching ? 'text-zinc-500 animate-pulse-dot' : 'text-zinc-600'}`}
            >
              {statusText}
            </p>
          </div>
          <ToggleSwitch
            checked={config.enabled && available && config.keys.length > 0}
            disabled={!available || busy || config.keys.length === 0}
            onChange={(enabled) => void setEnabled(enabled)}
            tone="amber"
          />
        </div>

        <SpammerKeyboard
          config={config}
          armed={status.armed}
          available={available}
          disabled={!server || busy}
          onKeysChange={(keys) => void updateField({ keys })}
        />

        <div className="flex items-center gap-2">
          <span className="text-[10px] text-zinc-600 uppercase tracking-wide shrink-0">
            Delay
          </span>
          <input
            type="range"
            min={minimumDelayMs}
            max={50}
            step={1}
            disabled={!server || busy}
            value={Math.max(config.delayMs, minimumDelayMs)}
            onChange={(event) =>
              void updateField({ delayMs: Number(event.target.value) })
            }
            className="flex-1 accent-amber-500 disabled:opacity-50"
          />
          <span className="text-[10px] text-zinc-500 w-8 text-right shrink-0">
            {status.active
              ? status.effectiveDelayMs
              : Math.max(config.delayMs, minimumDelayMs)}
            ms
          </span>
        </div>

        <GearSwitchEditor
          spammerKeys={config.keys}
          gear={config.gearSwitch}
          gearMode={status.gearMode}
          disabled={!server || busy}
          onChange={(gearSwitch) => void updateField({ gearSwitch })}
        />

        <p className="text-[10px] leading-snug min-h-[calc(1em*1.375)]">
          {error && available ? (
            <span className="text-red-400/90">{error}</span>
          ) : null}
        </p>
      </div>
    </Panel>
  )
}
