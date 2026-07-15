import { memo, useMemo, useState, type ReactNode } from 'react'
import { ChevronDown, Shield, Swords, X } from 'lucide-react'
import {
  GEAR_SWITCH_MAX_DELAY_MS,
  GEAR_SWITCH_MIN_DELAY_MS,
  SPAMMER_KEYS,
} from '../../shared/constants'
import type { GearSwitchConfig } from '../../shared/types'
import { DarkSelect } from '../../shared/ui/DarkSelect'
import { ToggleSwitch } from '../../shared/ui/ToggleSwitch'
import {
  addGearRule,
  removeGearRule,
  toggleGearRuleKey,
  type GearKeyField,
} from './spammer.logic'

type ChipTone = 'amber' | 'sky'

const CHIP_ACTIVE_CLASSES: Record<ChipTone, string> = {
  amber: 'border-amber-500/70 bg-amber-500/15 text-amber-200',
  sky: 'border-sky-500/70 bg-sky-500/15 text-sky-200',
}

const GEAR_TONE_LABEL: Record<ChipTone, string> = {
  amber: 'text-amber-400/80',
  sky: 'text-sky-400/80',
}

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
      SPAMMER_KEYS.filter((key) => !keys.includes(key)).map((key) => ({
        value: key,
        label: key,
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

interface GearSwitchEditorProps {
  spammerKeys: string[]
  gear: GearSwitchConfig
  gearMode?: 'atk' | 'def' | null
  disabled: boolean
  onChange: (gear: GearSwitchConfig) => void
}

export function GearSwitchEditor({
  spammerKeys,
  gear,
  gearMode,
  disabled,
  onChange,
}: GearSwitchEditorProps) {
  const [open, setOpen] = useState(false)
  const availableRuleTriggers = useMemo(
    () =>
      spammerKeys
        .filter((key) => !gear.rules.some((rule) => rule.trigger === key))
        .map((key) => ({ value: key, label: key })),
    [spammerKeys, gear.rules],
  )
  const patch = (value: Partial<GearSwitchConfig>) =>
    onChange({ ...gear, ...value })
  const toggleRuleKey = (trigger: string, field: GearKeyField, key: string) =>
    onChange(toggleGearRuleKey(gear, trigger, field, key))

  return (
    <div className="rounded-lg bg-zinc-950/40 border border-zinc-800/60">
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        className="w-full flex items-center justify-between gap-2 px-2.5 py-2 text-left"
      >
        <span className="flex items-center gap-1.5 text-[10px] uppercase tracking-wide text-zinc-500">
          <Swords className="w-3 h-3 shrink-0" aria-hidden />
          ATK / DEF Gear Switch
          {gear.enabled && (
            <span className="rounded bg-amber-500/15 px-1 text-[9px] font-semibold text-amber-300 normal-case tracking-normal">
              {gearMode === 'atk' ? 'ATK' : gearMode === 'def' ? 'DEF' : 'on'}
            </span>
          )}
        </span>
        <ChevronDown
          className={`w-3 h-3 text-zinc-600 transition-transform ${open ? 'rotate-180' : ''}`}
          aria-hidden
        />
      </button>

      {open && (
        <div className="space-y-2 px-2.5 pb-2.5">
          <div className="flex items-center justify-between gap-2">
            <p className="text-[10px] text-zinc-600 leading-snug">
              Al mantener la tecla del spammer equipa ATK; al soltarla, DEF.
            </p>
            <ToggleSwitch
              checked={gear.enabled}
              disabled={disabled}
              onChange={(enabled) => patch({ enabled })}
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
                    disabled={disabled || availableRuleTriggers.length === 0}
                    onChange={(trigger) => onChange(addGearRule(gear, trigger))}
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
                          disabled={disabled}
                          onClick={() =>
                            onChange(removeGearRule(gear, rule.trigger))
                          }
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
                          <Swords className="w-3 h-3 shrink-0" aria-hidden />
                        }
                        keys={rule.atkKeys}
                        disabled={disabled}
                        onToggle={(key) =>
                          toggleRuleKey(rule.trigger, 'atkKeys', key)
                        }
                      />
                      <GearKeySet
                        label="DEF"
                        tone="sky"
                        icon={
                          <Shield className="w-3 h-3 shrink-0" aria-hidden />
                        }
                        keys={rule.defKeys}
                        disabled={disabled}
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
                  disabled={disabled}
                  value={gear.switchDelayMs}
                  onChange={(event) =>
                    patch({ switchDelayMs: Number(event.target.value) })
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
  )
}
