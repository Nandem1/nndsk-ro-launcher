import { DEFAULT_AUTOPOT_CONFIG, POT_KEYS } from '../../shared/constants'
import { Panel } from '../../shared/ui/Panel'
import { DarkSelect } from '../../shared/ui/DarkSelect'
import { useSelectedServer } from '../servers/useSelectedServer'
import { statPercent } from './autopot.logic'
import { useAutopot } from './useAutopot'

function StatBar({
  label,
  cur,
  max,
  tone,
}: {
  label: string
  cur: number
  max: number
  tone: 'red' | 'blue'
}) {
  const pct = statPercent(cur, max)
  const gradient =
    tone === 'red'
      ? 'from-red-600 to-red-400'
      : 'from-sky-600 to-sky-400'

  return (
    <div className="space-y-1">
      <div className="flex justify-between text-[11px] text-zinc-500">
        <span>{label}</span>
        <span>
          {cur.toLocaleString()} / {max.toLocaleString()} ({pct}%)
        </span>
      </div>
      <div className="h-1.5 bg-zinc-800 rounded-full overflow-hidden">
        <div
          className={`h-full bg-gradient-to-r ${gradient} transition-all duration-300`}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  )
}

export function AutopotPanel() {
  const server = useSelectedServer()
  const { config, status, busy, isRunning, error, setEnabled, updateField } = useAutopot(server)
  const showProbeHint =
    config.enabled &&
    status.active &&
    status.maxHp === 0 &&
    !error

  if (!isRunning || !server) return null

  const toggleClass = config.enabled
    ? 'bg-emerald-500/15 text-emerald-300 border border-emerald-500/30'
    : 'bg-zinc-800 text-zinc-400 border border-zinc-700/80 hover:text-zinc-200'

  return (
    <Panel title="AutoPot" className="shrink-0 max-h-[280px] overflow-y-auto">
      <div className="space-y-2.5">
        <div className="flex items-center justify-between gap-3">
          <div className="min-w-0">
            <p className="text-xs text-zinc-300">
              {status.characterName || server.name}
            </p>
            <p className="text-[11px] text-zinc-600 truncate">
              {status.active ? 'Activo' : 'Inactivo'}
            </p>
          </div>
          <button
            type="button"
            disabled={busy}
            onClick={() => void setEnabled(!config.enabled)}
            className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors disabled:opacity-50 ${toggleClass}`}
          >
            {config.enabled ? 'ON' : 'OFF'}
          </button>
        </div>

        <div className="grid grid-cols-[56px_1fr_56px] gap-2 items-center">
          <span className="text-[11px] text-zinc-500 uppercase">HP</span>
          <DarkSelect
            value={config.hpKey}
            onChange={(hpKey) => void updateField({ hpKey })}
            options={POT_KEYS.map((key) => ({ value: key, label: key }))}
          />
          <input
            type="number"
            min={1}
            max={99}
            value={config.hpPercent}
            onChange={(e) =>
              void updateField({
                hpPercent: Number(e.target.value) || DEFAULT_AUTOPOT_CONFIG.hpPercent,
              })
            }
            className="w-full bg-zinc-950/60 border border-zinc-700/80 rounded-lg px-2 py-1.5 text-xs text-zinc-200"
          />
        </div>

        <div className="grid grid-cols-[56px_1fr_56px] gap-2 items-center">
          <span className="text-[11px] text-zinc-500 uppercase">SP</span>
          <DarkSelect
            value={config.spKey}
            onChange={(spKey) => void updateField({ spKey })}
            options={POT_KEYS.map((key) => ({ value: key, label: key }))}
          />
          <input
            type="number"
            min={1}
            max={99}
            value={config.spPercent}
            onChange={(e) =>
              void updateField({
                spPercent: Number(e.target.value) || DEFAULT_AUTOPOT_CONFIG.spPercent,
              })
            }
            className="w-full bg-zinc-950/60 border border-zinc-700/80 rounded-lg px-2 py-1.5 text-xs text-zinc-200"
          />
        </div>

        {(status.active || config.enabled) && (
          <div className="space-y-2 pt-1">
            <StatBar label="HP" cur={status.curHp} max={status.maxHp} tone="red" />
            <StatBar label="SP" cur={status.curSp} max={status.maxSp} tone="blue" />
          </div>
        )}

        {error && (
          <p className="text-[11px] text-red-400 leading-relaxed">{error}</p>
        )}

        {showProbeHint && (
          <p className="text-[11px] text-amber-500/90 leading-relaxed">
            HP/SP en cero — revisa la pestaña Tools en Logs (PID, probe, ptrace).
          </p>
        )}
      </div>
    </Panel>
  )
}
