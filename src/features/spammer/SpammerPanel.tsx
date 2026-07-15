import { memo, useMemo, useCallback, useState, type ReactNode } from 'react'
import { Zap, Shield, Swords, ChevronDown, X } from 'lucide-react'
import {
  GEAR_SWITCH_MAX_DELAY_MS,
  GEAR_SWITCH_MIN_DELAY_MS,
  SPAMMER_FUNCTION_KEYS,
  SPAMMER_KEYS,
  SPAMMER_LETTER_KEY_ROWS,
  SPAMMER_NUMBER_KEYS,
} from '../../shared/constants'
import { Panel, resolveToolTone } from '../../shared/ui/Panel'
import { DarkSelect } from '../../shared/ui/DarkSelect'
import { ToggleSwitch } from '../../shared/ui/ToggleSwitch'
import { useSelectedServer } from '../servers/useSelectedServer'
import { useLauncherStore } from '../launcher/launcher.store'
import { useUiModeStore } from '../../app/uiMode.store'
import {
  addGearRule,
  formatSpammerKeys,
  removeGearRule,
  toggleGearRuleKey,
  toggleSpammerKey,
  type GearKeyField,
} from './spammer.logic'
import { useSpammer } from './useSpammer'

type ChipTone = 'amber' | 'sky'

const CHIP_ACTIVE_CLASSES: Record<ChipTone, string> = {
  amber: 'border-amber-500/70 bg-amber-500/15 text-amber-200',
  sky: 'border-sky-500/70 bg-sky-500/15 text-sky-200',
}

const KeyChip = memo(function KeyChip({
  label,
  active,
  disabled,
  onToggle,
  tone = 'amber',
}: {
  label: string
  active: boolean
  disabled: boolean
  onToggle: () => void
  tone?: ChipTone
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onToggle}
      className={`min-w-0 flex-1 px-1 py-1 rounded-md text-[10px] font-semibold border transition-colors motion-safe:active:scale-[0.97] disabled:opacity-40 ${
        active
          ? CHIP_ACTIVE_CLASSES[tone]
          : 'border-zinc-800/80 bg-zinc-950/50 text-zinc-600 hover:text-zinc-400'
      }`}
    >
      {label}
    </button>
  )
})

const GEAR_TONE_LABEL: Record<ChipTone, string> = {
  amber: 'text-amber-400/80',
  sky: 'text-sky-400/80',
}

/** Editor de un set de equipo: chips removibles + selector para añadir teclas. */
const GearKeySet = memo(function GearKeySet({
  label,
  icon,
  tone,
  keys,
  disabled,
  onToggle,
}: {
  label: string
  icon: ReactNode
  tone: ChipTone
  keys: string[]
  disabled: boolean
  onToggle: (key: string) => void
}) {
  const available = useMemo(
    () =>
      SPAMMER_KEYS.filter((k) => !keys.includes(k)).map((k) => ({
        value: k,
        label: k,
      })),
    [keys],
  )

  return (
    <div className="flex items-center gap-2">
      <span
        className={`flex w-10 shrink-0 items-center gap-1 text-[10px] font-semibold uppercase tracking-wide ${GEAR_TONE_LABEL[tone]}`}
      >
        {icon} {label}
      </span>
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1">
        {keys.length === 0 && (
          <span className="text-[10px] text-zinc-600">Sin equipo</span>
        )}
        {keys.map((key) => (
          <button
            key={key}
            type="button"
            disabled={disabled}
            onClick={() => onToggle(key)}
            className={`inline-flex items-center gap-0.5 rounded-md border px-1.5 py-0.5 text-[10px] font-semibold transition-colors disabled:opacity-40 ${CHIP_ACTIVE_CLASSES[tone]}`}
            aria-label={`Quitar tecla ${key}`}
          >
            {key}
            <X className="h-2.5 w-2.5 opacity-70" />
          </button>
        ))}
        <div className="w-[68px] shrink-0">
          <DarkSelect
            compact
            value=""
            placeholder="+ tecla"
            options={available}
            disabled={disabled || available.length === 0}
            onChange={onToggle}
          />
        </div>
      </div>
    </div>
  )
})

export function SpammerPanel() {
  const server = useSelectedServer()
  const { config, status, busy, isRunning, error, setEnabled, updateField } =
    useSpammer(server)
  const launching = useLauncherStore((s) => s.status === 'launching')
  const hero = useUiModeStore((s) => s.mode === 'ingame')

  const available = isRunning && !!server
  const selectedKeys = useMemo(() => new Set(config.keys), [config.keys])
  const keysLabel = useMemo(() => formatSpammerKeys(config.keys), [config.keys])

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

  const toggleKey = useCallback(
    (key: string) => {
      const next = toggleSpammerKey(config, key)
      void updateField({ keys: next.keys })
    },
    [config, updateField],
  )

  const gear = config.gearSwitch
  const [gearOpen, setGearOpen] = useState(false)
  const availableRuleTriggers = useMemo(
    () =>
      config.keys
        .filter((key) => !gear.rules.some((rule) => rule.trigger === key))
        .map((key) => ({ value: key, label: key })),
    [config.keys, gear.rules],
  )

  const patchGear = useCallback(
    (patch: Partial<typeof gear>) =>
      void updateField({ gearSwitch: { ...gear, ...patch } }),
    [gear, updateField],
  )
  const addRule = useCallback(
    (trigger: string) =>
      void updateField({ gearSwitch: addGearRule(gear, trigger) }),
    [gear, updateField],
  )
  const removeRule = useCallback(
    (trigger: string) =>
      void updateField({ gearSwitch: removeGearRule(gear, trigger) }),
    [gear, updateField],
  )
  const toggleRuleKey = useCallback(
    (trigger: string, field: GearKeyField, key: string) =>
      void updateField({
        gearSwitch: toggleGearRuleKey(gear, trigger, field, key),
      }),
    [gear, updateField],
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

        <div className="space-y-1.5 rounded-lg bg-zinc-950/40 border border-zinc-800/60 px-2.5 py-2">
          <div className="flex justify-between text-[10px]">
            <span className="text-zinc-600 uppercase tracking-wide">
              Teclas
            </span>
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
          <div className="space-y-1">
            {[SPAMMER_FUNCTION_KEYS, SPAMMER_NUMBER_KEYS].map(
              (row, rowIndex) => (
                <div key={rowIndex} className="flex gap-1">
                  {row.map((key) => (
                    <KeyChip
                      key={key}
                      label={key}
                      active={selectedKeys.has(key)}
                      disabled={!server || busy}
                      onToggle={() => toggleKey(key)}
                    />
                  ))}
                </div>
              ),
            )}
            <div className="space-y-1 pt-0.5">
              {SPAMMER_LETTER_KEY_ROWS.map((row, rowIndex) => (
                <div
                  key={rowIndex}
                  className={`flex gap-1 ${
                    rowIndex === 1
                      ? 'px-[5%]'
                      : rowIndex === 2
                        ? 'px-[15%]'
                        : ''
                  }`}
                >
                  {row.map((key) => (
                    <KeyChip
                      key={key}
                      label={key}
                      active={selectedKeys.has(key)}
                      disabled={!server || busy}
                      onToggle={() => toggleKey(key)}
                    />
                  ))}
                </div>
              ))}
            </div>
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
            onChange={(e) =>
              void updateField({ delayMs: Number(e.target.value) })
            }
            className="flex-1 accent-amber-500 disabled:opacity-50"
          />
          <span className="text-[10px] text-zinc-500 w-8 text-right shrink-0">
            {config.delayMs}ms
          </span>
        </div>

        <div className="rounded-lg bg-zinc-950/40 border border-zinc-800/60">
          <button
            type="button"
            onClick={() => setGearOpen((v) => !v)}
            className="w-full flex items-center justify-between gap-2 px-2.5 py-2 text-left"
          >
            <span className="flex items-center gap-1.5 text-[10px] uppercase tracking-wide text-zinc-500">
              <Swords className="w-3 h-3 shrink-0" aria-hidden />
              ATK / DEF Gear Switch
              {gear.enabled && (
                <span className="rounded bg-amber-500/15 px-1 text-[9px] font-semibold text-amber-300 normal-case tracking-normal">
                  {status.gearMode === 'atk'
                    ? 'ATK'
                    : status.gearMode === 'def'
                      ? 'DEF'
                      : 'on'}
                </span>
              )}
            </span>
            <ChevronDown
              className={`w-3 h-3 text-zinc-600 transition-transform ${gearOpen ? 'rotate-180' : ''}`}
              aria-hidden
            />
          </button>

          {gearOpen && (
            <div className="space-y-2 px-2.5 pb-2.5">
              <div className="flex items-center justify-between gap-2">
                <p className="text-[10px] text-zinc-600 leading-snug">
                  Al mantener la tecla del spammer equipa ATK; al soltarla, DEF.
                </p>
                <ToggleSwitch
                  checked={gear.enabled}
                  disabled={!server || busy}
                  onChange={(enabled) => patchGear({ enabled })}
                  tone="amber"
                />
              </div>

              {gear.enabled && (
                <>
                  <div className="flex items-center gap-2 border-t border-zinc-800/60 pt-2">
                    <span className="shrink-0 text-[10px] uppercase tracking-wide text-zinc-600">
                      Agregar trigger
                    </span>
                    <div className="min-w-0 flex-1">
                      <DarkSelect
                        compact
                        keycap
                        value=""
                        placeholder={
                          availableRuleTriggers.length > 0
                            ? '+ regla'
                            : 'Todos configurados'
                        }
                        options={availableRuleTriggers}
                        disabled={
                          !server || busy || availableRuleTriggers.length === 0
                        }
                        onChange={addRule}
                      />
                    </div>
                  </div>

                  {gear.rules.length === 0 ? (
                    <p className="rounded-md border border-dashed border-zinc-800 px-2 py-2 text-center text-[10px] text-zinc-600">
                      Agrega una tecla del spammer y define su equipo ATK / DEF.
                    </p>
                  ) : (
                    <div className="space-y-2">
                      {gear.rules.map((rule) => (
                        <div
                          key={rule.trigger}
                          className="space-y-1.5 rounded-lg border border-zinc-800/80 bg-zinc-900/35 px-2 py-2"
                        >
                          <div className="flex items-center justify-between gap-2">
                            <span className="text-[10px] uppercase tracking-wide text-zinc-600">
                              Trigger{' '}
                              <span className="ml-1 rounded border border-amber-500/30 bg-amber-500/[0.08] px-1.5 py-0.5 font-semibold text-amber-200">
                                {rule.trigger}
                              </span>
                            </span>
                            <button
                              type="button"
                              disabled={!server || busy}
                              onClick={() => removeRule(rule.trigger)}
                              className="rounded p-0.5 text-zinc-600 transition-colors hover:bg-red-500/10 hover:text-red-300 disabled:opacity-40"
                              aria-label={`Eliminar regla ${rule.trigger}`}
                            >
                              <X className="h-3 w-3" />
                            </button>
                          </div>
                          <GearKeySet
                            label="ATK"
                            tone="amber"
                            icon={
                              <Swords
                                className="w-3 h-3 shrink-0"
                                aria-hidden
                              />
                            }
                            keys={rule.atkKeys}
                            disabled={!server || busy}
                            onToggle={(key) =>
                              toggleRuleKey(rule.trigger, 'atkKeys', key)
                            }
                          />
                          <GearKeySet
                            label="DEF"
                            tone="sky"
                            icon={
                              <Shield
                                className="w-3 h-3 shrink-0"
                                aria-hidden
                              />
                            }
                            keys={rule.defKeys}
                            disabled={!server || busy}
                            onToggle={(key) =>
                              toggleRuleKey(rule.trigger, 'defKeys', key)
                            }
                          />
                        </div>
                      ))}
                    </div>
                  )}

                  <div className="flex items-center gap-2">
                    <span className="text-[10px] text-zinc-600 uppercase tracking-wide shrink-0">
                      Switch
                    </span>
                    <input
                      type="range"
                      min={GEAR_SWITCH_MIN_DELAY_MS}
                      max={GEAR_SWITCH_MAX_DELAY_MS}
                      step={5}
                      disabled={!server || busy}
                      value={gear.switchDelayMs}
                      onChange={(e) =>
                        patchGear({ switchDelayMs: Number(e.target.value) })
                      }
                      className="flex-1 accent-amber-500 disabled:opacity-50"
                    />
                    <span className="text-[10px] text-zinc-500 w-10 text-right shrink-0">
                      {gear.switchDelayMs}ms
                    </span>
                  </div>
                </>
              )}
            </div>
          )}
        </div>

        <p className="text-[10px] leading-snug min-h-[calc(1em*1.375)]">
          {error && available ? (
            <span className="text-red-400/90">{error}</span>
          ) : null}
        </p>
      </div>
    </Panel>
  )
}
