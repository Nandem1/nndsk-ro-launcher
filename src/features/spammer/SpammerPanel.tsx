import { POT_KEYS } from '../../shared/constants'
import { Panel, type PanelTone } from '../../shared/ui/Panel'
import { ToggleSwitch } from '../../shared/ui/ToggleSwitch'
import { useSelectedServer } from '../servers/useSelectedServer'
import { formatSpammerKeys, toggleSpammerKey } from './spammer.logic'
import { useSpammer } from './useSpammer'

function resolveTone(
  available: boolean,
  enabled: boolean,
  armed: boolean,
  hasError: boolean,
): PanelTone {
  if (hasError) return 'danger'
  if (!available) return 'idle'
  if (enabled && armed) return 'warning'
  return 'neutral'
}

function KeyChip({
  label,
  active,
  disabled,
  onToggle,
}: {
  label: string
  active: boolean
  disabled: boolean
  onToggle: () => void
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onToggle}
      className={`min-w-[2rem] px-1.5 py-1 rounded-md text-[10px] font-semibold border transition-colors disabled:opacity-40 ${
        active
          ? 'border-amber-500/70 bg-amber-500/15 text-amber-200'
          : 'border-zinc-800/80 bg-zinc-950/50 text-zinc-600 hover:text-zinc-400'
      }`}
    >
      {label}
    </button>
  )
}

export function SpammerPanel() {
  const server = useSelectedServer()
  const { config, status, busy, isRunning, error, setEnabled, updateField } =
    useSpammer(server)

  const available = isRunning && !!server
  const keysLabel = formatSpammerKeys(config.keys)
  const selectedKeys = new Set(config.keys)

  const statusLabel = (() => {
    if (!available) return 'Inactivo'
    if (!status.armed) return 'Inactivo'
    if (status.spamming && status.key) {
      return `${status.cycleCount.toLocaleString()} ciclos · ${status.key} + click`
    }
    return `Standby — ${keysLabel}`
  })()

  const statusText = !server
    ? 'Selecciona un servidor'
    : !isRunning
      ? 'Inicia el juego'
      : config.keys.length === 0
        ? 'Selecciona al menos una tecla'
        : status.spamming
          ? 'Spameando...'
          : 'Mantén una tecla configurada en el juego'

  const tone = resolveTone(available, config.enabled, status.armed, !!error)

  const toggleKey = (key: string) => {
    const next = toggleSpammerKey(config, key)
    void updateField({ keys: next.keys })
  }

  return (
    <Panel title="Spammer" compact tone={tone} className="h-full">
      <div className="space-y-2">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <p
              className={`text-sm font-semibold truncate ${
                status.spamming ? 'text-amber-200' : 'text-zinc-100'
              }`}
            >
              {statusLabel}
            </p>
            <p className="text-[10px] text-zinc-600">{statusText}</p>
          </div>
          <ToggleSwitch
            checked={config.enabled && available && config.keys.length > 0}
            disabled={!available || busy || config.keys.length === 0}
            onChange={(enabled) => void setEnabled(enabled)}
            tone="amber"
          />
        </div>

        <div className="space-y-1.5 rounded-lg bg-zinc-950/40 border border-zinc-800/60 px-2.5 py-2">
          <div className="flex justify-between text-[10px]">
            <span className="text-zinc-600">Teclas</span>
            <span
              className={
                available && status.armed
                  ? 'text-amber-400/90 font-medium truncate ml-2'
                  : 'text-zinc-700 truncate ml-2'
              }
            >
              {keysLabel}
            </span>
          </div>
          <div className="flex flex-wrap gap-1">
            {POT_KEYS.map((key) => (
              <KeyChip
                key={key}
                label={key}
                active={selectedKeys.has(key)}
                disabled={!server || busy}
                onToggle={() => toggleKey(key)}
              />
            ))}
          </div>
          <p className="text-[10px] text-zinc-600 leading-snug">
            Skill en barra + target con click izquierdo
          </p>
        </div>

        <div className="flex items-center gap-2">
          <span className="text-[10px] text-zinc-600 uppercase tracking-wide shrink-0">
            Delay
          </span>
          <input
            type="range"
            min={5}
            max={50}
            step={1}
            disabled={!server || busy}
            value={config.delayMs}
            onChange={(e) => void updateField({ delayMs: Number(e.target.value) })}
            className="flex-1 accent-amber-500 disabled:opacity-50"
          />
          <span className="text-[10px] text-zinc-500 w-8 text-right shrink-0">
            {config.delayMs}ms
          </span>
        </div>

        {error && available ? (
          <p className="text-[10px] text-red-400/90 leading-snug">{error}</p>
        ) : null}
      </div>
    </Panel>
  )
}
