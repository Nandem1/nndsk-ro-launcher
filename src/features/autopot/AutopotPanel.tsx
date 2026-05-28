import { useEffect, useRef, useState } from 'react'
import { DEFAULT_AUTOPOT_CONFIG, POT_KEYS } from '../../shared/constants'

const POT_KEY_OPTIONS = POT_KEYS.map((key) => ({ value: key, label: key }))
import { Panel, type PanelTone } from '../../shared/ui/Panel'
import { DarkSelect } from '../../shared/ui/DarkSelect'
import { ToggleSwitch } from '../../shared/ui/ToggleSwitch'
import { useSelectedServer } from '../servers/useSelectedServer'
import { statPercent } from './autopot.logic'
import { useAutopot } from './useAutopot'

function StatBar({
  cur,
  max,
  tone,
  flash,
}: {
  cur: number
  max: number
  tone: 'red' | 'blue'
  flash?: boolean
}) {
  const empty = max <= 0
  const pct = statPercent(cur, max)
  const gradient =
    tone === 'red'
      ? 'from-red-600 to-red-400'
      : 'from-sky-600 to-sky-400'

  const flashClass = flash
    ? tone === 'red'
      ? 'animate-stat-flash-red'
      : 'animate-stat-flash-blue'
    : ''

  return (
    <div className={`space-y-0.5 ${flashClass}`}>
      <div className={`flex justify-between text-[10px] ${empty ? 'text-zinc-700' : 'text-zinc-500'}`}>
        <span>{tone === 'red' ? 'HP' : 'SP'}</span>
        <span>
          {empty
            ? '— / —'
            : `${cur.toLocaleString()} / ${max.toLocaleString()} (${pct}%)`}
        </span>
      </div>
      <div className={`h-2 rounded-full overflow-hidden ${empty ? 'bg-zinc-900/80 border border-dashed border-zinc-800/80' : 'bg-zinc-800'}`}>
        {!empty && (
          <div
            className={`h-full bg-gradient-to-r ${gradient} transition-all duration-300`}
            style={{ width: `${pct}%` }}
          />
        )}
      </div>
    </div>
  )
}

function resolveTone(
  available: boolean,
  enabled: boolean,
  active: boolean,
  hasError: boolean,
): PanelTone {
  if (hasError) return 'danger'
  if (!available) return 'idle'
  if (enabled && active) return 'success'
  return 'neutral'
}

export function AutopotPanel() {
  const server = useSelectedServer()
  const { config, status, busy, isRunning, error, setEnabled, updateField } = useAutopot(server)
  const available = isRunning && !!server
  const hasCharacter = available && !!status.characterName
  const [flashHp, setFlashHp] = useState(false)
  const [flashSp, setFlashSp] = useState(false)
  const prevHp = useRef(0)
  const prevSp = useRef(0)

  const showProbeHint =
    available &&
    config.enabled &&
    status.active &&
    status.maxHp === 0 &&
    !error

  useEffect(() => {
    if (!available || status.maxHp <= 0) {
      prevHp.current = 0
      return
    }
    if (status.curHp > prevHp.current && prevHp.current > 0) {
      setFlashHp(true)
      const t = setTimeout(() => setFlashHp(false), 450)
      prevHp.current = status.curHp
      return () => clearTimeout(t)
    }
    prevHp.current = status.curHp
  }, [available, status.curHp, status.maxHp])

  useEffect(() => {
    if (!available || status.maxSp <= 0) {
      prevSp.current = 0
      return
    }
    if (status.curSp > prevSp.current && prevSp.current > 0) {
      setFlashSp(true)
      const t = setTimeout(() => setFlashSp(false), 450)
      prevSp.current = status.curSp
      return () => clearTimeout(t)
    }
    prevSp.current = status.curSp
  }, [available, status.curSp, status.maxSp])

  const displayName = available
    ? status.characterName || server!.name
    : server?.name ?? 'Sin servidor'

  const statusText = !server
    ? 'Selecciona un servidor'
    : !isRunning
      ? 'Inicia el juego'
      : status.active
        ? 'Activo'
        : 'Inactivo'

  const hpCur = available && (status.active || config.enabled) ? status.curHp : 0
  const hpMax = available && (status.active || config.enabled) ? status.maxHp : 0
  const spCur = available && (status.active || config.enabled) ? status.curSp : 0
  const spMax = available && (status.active || config.enabled) ? status.maxSp : 0

  const tone = resolveTone(available, config.enabled, status.active, !!error)

  return (
    <Panel title="AutoPot" compact tone={tone} className="h-full">
      <div className="space-y-2">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <p className={`truncate text-sm font-semibold ${hasCharacter ? 'text-amber-100/95' : 'text-zinc-100'}`}>
              {displayName}
            </p>
            <p className="text-[10px] text-zinc-600">{statusText}</p>
          </div>
          <ToggleSwitch
            checked={config.enabled && available}
            disabled={!available || busy}
            onChange={(enabled) => void setEnabled(enabled)}
            tone="emerald"
          />
        </div>

        <div className="space-y-1.5 rounded-lg bg-zinc-950/40 border border-zinc-800/60 px-2.5 py-2">
          <StatBar cur={hpCur} max={hpMax} tone="red" flash={flashHp} />
          <StatBar cur={spCur} max={spMax} tone="blue" flash={flashSp} />
        </div>

        <div className="grid grid-cols-2 gap-1.5">
          <div className="space-y-1">
            <span className="text-[10px] text-zinc-600 uppercase tracking-wide">HP</span>
            <div className="flex gap-1">
              <DarkSelect
                value={config.hpKey}
                disabled={!server}
                onChange={(hpKey) => void updateField({ hpKey })}
                options={POT_KEY_OPTIONS}
              />
              <input
                type="number"
                min={1}
                max={99}
                disabled={!server}
                value={config.hpPercent}
                onChange={(e) =>
                  void updateField({
                    hpPercent: Number(e.target.value) || DEFAULT_AUTOPOT_CONFIG.hpPercent,
                  })
                }
                className="w-11 bg-zinc-950/60 border border-zinc-700/80 rounded-md px-1.5 py-1 text-[11px] text-zinc-200 text-center disabled:opacity-50"
              />
            </div>
          </div>
          <div className="space-y-1">
            <span className="text-[10px] text-zinc-600 uppercase tracking-wide">SP</span>
            <div className="flex gap-1">
              <DarkSelect
                value={config.spKey}
                disabled={!server}
                onChange={(spKey) => void updateField({ spKey })}
                options={POT_KEY_OPTIONS}
              />
              <input
                type="number"
                min={1}
                max={99}
                disabled={!server}
                value={config.spPercent}
                onChange={(e) =>
                  void updateField({
                    spPercent: Number(e.target.value) || DEFAULT_AUTOPOT_CONFIG.spPercent,
                  })
                }
                className="w-11 bg-zinc-950/60 border border-zinc-700/80 rounded-md px-1.5 py-1 text-[11px] text-zinc-200 text-center disabled:opacity-50"
              />
            </div>
          </div>
        </div>

        <p className="text-[10px] leading-snug min-h-[calc(1em*1.375)]">
          {error && available
            ? <span className="text-red-400">{error}</span>
            : showProbeHint
              ? <span className="text-amber-500/90">HP/SP en cero — revisa Tools en Logs.</span>
              : null}
        </p>
      </div>
    </Panel>
  )
}
