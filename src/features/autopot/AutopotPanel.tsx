import { useEffect, useRef, useState } from 'react'
import { FlaskConical } from 'lucide-react'
import { DEFAULT_AUTOPOT_CONFIG, POT_KEYS } from '../../shared/constants'

const POT_KEY_OPTIONS = POT_KEYS.map((key) => ({ value: key, label: key }))
import { Panel, resolveToolTone } from '../../shared/ui/Panel'
import { DarkSelect } from '../../shared/ui/DarkSelect'
import { ToggleSwitch } from '../../shared/ui/ToggleSwitch'
import { useSelectedServer } from '../servers/useSelectedServer'
import { useLauncherStore } from '../launcher/launcher.store'
import { useUiModeStore } from '../../app/uiMode.store'
import { statPercent } from './autopot.logic'
import { useAutopot } from './useAutopot'
import { api } from '../../shared/api'
import type { ClientProfile } from '../../shared/types'

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
    tone === 'red' ? 'from-red-600 to-red-400' : 'from-sky-600 to-sky-400'

  const flashClass = flash
    ? tone === 'red'
      ? 'animate-stat-flash-red'
      : 'animate-stat-flash-blue'
    : ''

  return (
    <div className={`space-y-0.5 ${flashClass}`}>
      <div
        className={`flex justify-between text-[10px] ${empty ? 'text-zinc-700' : 'text-zinc-500'}`}
      >
        <span>{tone === 'red' ? 'HP' : 'SP'}</span>
        <span>
          {empty
            ? '— / —'
            : `${cur.toLocaleString()} / ${max.toLocaleString()} (${pct}%)`}
        </span>
      </div>
      <div
        className={`h-2 rounded-full overflow-hidden ${empty ? 'bg-zinc-900/80 border border-dashed border-zinc-800/80' : 'bg-zinc-800'}`}
      >
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

export function AutopotPanel() {
  const server = useSelectedServer()
  const { config, status, busy, isRunning, error, setEnabled, updateField } =
    useAutopot(server)
  const launching = useLauncherStore((s) => s.status === 'launching')
  const hero = useUiModeStore((s) => s.mode === 'ingame')
  const available = isRunning && !!server
  const minimumDelayMs = 10
  const hasCharacter = available && !!status.characterName
  const [flashHp, setFlashHp] = useState(false)
  const [flashSp, setFlashSp] = useState(false)
  const prevHp = useRef(0)
  const prevSp = useRef(0)
  const [profiles, setProfiles] = useState<ClientProfile[]>([])

  useEffect(() => {
    void api.listClientProfiles().then(setProfiles).catch(console.error)
  }, [])

  const showProbeHint =
    available && config.enabled && status.active && status.maxHp === 0 && !error

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
    : (server?.name ?? 'Sin servidor')

  const statusText = !server
    ? 'Selecciona un servidor'
    : launching
      ? 'Iniciando juego...'
      : !isRunning
        ? 'Inicia el juego'
        : status.active
          ? 'Activo'
          : 'Inactivo'

  const hpCur =
    available && (status.active || config.enabled) ? status.curHp : 0
  const hpMax =
    available && (status.active || config.enabled) ? status.maxHp : 0
  const spCur =
    available && (status.active || config.enabled) ? status.curSp : 0
  const spMax =
    available && (status.active || config.enabled) ? status.maxSp : 0

  const tone = resolveToolTone(
    available,
    config.enabled && status.active,
    !!error,
  )

  return (
    <Panel
      title="AutoPot"
      compact
      hero={hero}
      tone={tone}
      className="h-full"
      leading={
        <FlaskConical className="w-3 h-3 text-zinc-600 shrink-0" aria-hidden />
      }
    >
      <div className="flex-1 min-h-0 overflow-y-auto space-y-2 pr-0.5">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <p
              className={`truncate text-sm font-semibold ${hasCharacter ? 'text-amber-100/95' : 'text-zinc-100'}`}
            >
              {displayName}
            </p>
            <p
              className={`text-[10px] ${launching ? 'text-zinc-500 animate-pulse-dot' : 'text-zinc-600'}`}
            >
              {statusText}
            </p>
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

        <div className="flex items-center justify-between gap-2 rounded-lg border border-amber-500/15 bg-amber-500/5 px-2.5 py-2">
          <div className="min-w-0">
            <p className="text-[11px] font-medium text-amber-100/90">
              Modo proactivo
            </p>
            <p className="text-[10px] leading-snug text-zinc-500">
              Envía HP entre recuperaciones para reducir la reacción con
              latencia alta.
            </p>
          </div>
          <ToggleSwitch
            checked={config.proactiveMode}
            disabled={!server || busy}
            onChange={(proactiveMode) => void updateField({ proactiveMode })}
            tone="amber"
          />
        </div>

        <div className="flex items-center gap-2">
          <span className="text-[10px] text-zinc-600 uppercase tracking-wide shrink-0">
            Lectura
          </span>
          <input
            type="range"
            min={minimumDelayMs}
            max={200}
            step={1}
            disabled={!server || busy}
            value={Math.max(config.delayMs, minimumDelayMs)}
            onChange={(event) =>
              void updateField({ delayMs: Number(event.target.value) })
            }
            className="flex-1 accent-amber-500 disabled:opacity-50"
          />
          <span className="text-[10px] text-zinc-500 w-10 text-right shrink-0">
            {status.active
              ? status.effectiveDelayMs
              : Math.max(config.delayMs, minimumDelayMs)}
            ms
          </span>
        </div>

        <div className="space-y-1">
          <span className="text-[10px] text-zinc-600 uppercase tracking-wide">
            Perfil de memoria
          </span>
          <DarkSelect
            compact
            value={config.profileId ?? ''}
            disabled={!server}
            onChange={(val) =>
              void updateField({ profileId: val || undefined })
            }
            options={[
              { value: '', label: 'Auto' },
              ...profiles.map((p) => ({ value: p.id, label: p.label })),
            ]}
          />
        </div>

        <div className="grid grid-cols-2 gap-1.5">
          <div className="space-y-1">
            <span className="text-[10px] text-zinc-600 uppercase tracking-wide">
              HP
            </span>
            <div className="flex gap-1">
              <DarkSelect
                compact
                keycap
                value={config.hpKey}
                disabled={!server}
                onChange={(hpKey) => void updateField({ hpKey })}
                options={POT_KEY_OPTIONS}
              />
              <div className="relative w-12 shrink-0">
                <input
                  type="number"
                  min={1}
                  max={99}
                  inputMode="numeric"
                  aria-label="Porcentaje de HP"
                  disabled={!server}
                  value={config.hpPercent}
                  onChange={(e) =>
                    void updateField({
                      hpPercent:
                        Number(e.target.value) ||
                        DEFAULT_AUTOPOT_CONFIG.hpPercent,
                    })
                  }
                  className="input-no-spinner w-full rounded-md border border-zinc-700/80 bg-zinc-950/60 py-1 pl-1.5 pr-4 text-center text-[11px] text-zinc-200 outline-none transition-colors focus:border-amber-500/60 focus:ring-1 focus:ring-amber-500/20 disabled:opacity-50"
                />
                <span className="pointer-events-none absolute inset-y-0 right-1.5 flex items-center text-[9px] text-zinc-600">
                  %
                </span>
              </div>
            </div>
          </div>
          <div className="space-y-1">
            <span className="text-[10px] text-zinc-600 uppercase tracking-wide">
              SP
            </span>
            <div className="flex gap-1">
              <DarkSelect
                compact
                keycap
                value={config.spKey}
                disabled={!server}
                onChange={(spKey) => void updateField({ spKey })}
                options={POT_KEY_OPTIONS}
              />
              <div className="relative w-12 shrink-0">
                <input
                  type="number"
                  min={1}
                  max={99}
                  inputMode="numeric"
                  aria-label="Porcentaje de SP"
                  disabled={!server}
                  value={config.spPercent}
                  onChange={(e) =>
                    void updateField({
                      spPercent:
                        Number(e.target.value) ||
                        DEFAULT_AUTOPOT_CONFIG.spPercent,
                    })
                  }
                  className="input-no-spinner w-full rounded-md border border-zinc-700/80 bg-zinc-950/60 py-1 pl-1.5 pr-4 text-center text-[11px] text-zinc-200 outline-none transition-colors focus:border-amber-500/60 focus:ring-1 focus:ring-amber-500/20 disabled:opacity-50"
                />
                <span className="pointer-events-none absolute inset-y-0 right-1.5 flex items-center text-[9px] text-zinc-600">
                  %
                </span>
              </div>
            </div>
          </div>
        </div>

        <p className="text-[10px] leading-snug min-h-[calc(1em*1.375)]">
          {error && available ? (
            <span className="text-red-400/90">{error}</span>
          ) : showProbeHint ? (
            <span className="text-amber-500/90">
              HP/SP en cero — revisa Tools en Logs.
            </span>
          ) : null}
        </p>
      </div>
    </Panel>
  )
}
