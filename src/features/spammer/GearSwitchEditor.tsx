import { Input } from '../../shared/ui/Input'
import { memo, useMemo, useState } from 'react'
import { ChevronDown, X } from 'lucide-react'
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

type ChipTone = 'selection' | 'info'

const CHIP_ACTIVE_CLASSES: Record<ChipTone, string> = {
  selection: 'border-accent bg-accent text-on-accent',
  info: 'border-accent bg-accent text-on-accent',
}

const GEAR_TONE_LABEL: Record<ChipTone, string> = {
  selection: 'text-muted',
  info: 'text-muted',
}

const GearKeySet = memo(function GearKeySet({
  label,
  tone,
  keys,
  disabled,
  onToggle,
}: {
  label: string
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
        className={`flex w-10 shrink-0 items-center gap-1 micro-label ${GEAR_TONE_LABEL[tone]}`}
      >
        {label}
      </span>
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1">
        {keys.length === 0 && (
          <span className="text-caption text-muted">Sin equipo</span>
        )}
        {keys.map((key) => (
          <button
            key={key}
            type="button"
            disabled={disabled}
            onClick={() => onToggle(key)}
            className={`inline-flex items-center gap-0.5 rounded-control-compact font-mono border px-1.5 py-0.5 text-caption font-semibold transition-colors idle-control ${CHIP_ACTIVE_CLASSES[tone]}`}
            aria-label={`Quitar tecla ${key}`}
          >
            {key}
            <X className="h-2.5 w-2.5" />
          </button>
        ))}
        <div className="w-[68px] shrink-0">
          <DarkSelect
            size="sm"
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
    <div className="border-t border-line">
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        className="w-full flex items-center justify-between gap-2 py-2 text-left"
      >
        <span className="flex items-center gap-1.5 micro-label">
          ATK / DEF Gear Switch
          {gear.enabled && (
            <span className="px-1 text-micro font-semibold text-ok normal-case tracking-normal">
              {gearMode === 'atk' ? 'ATK' : gearMode === 'def' ? 'DEF' : 'on'}
            </span>
          )}
        </span>
        <ChevronDown
          className={`w-3 h-3 text-muted transition-colors ${open ? 'rotate-180' : ''}`}
          aria-hidden
        />
      </button>

      {open && (
        <div className="space-y-2 pb-2.5">
          <div className="flex items-center justify-between gap-2">
            <p className="text-caption text-muted leading-snug">
              Al mantener la tecla del spammer equipa ATK; al soltarla, DEF.
            </p>
            <ToggleSwitch
              checked={gear.enabled}
              disabled={disabled}
              onChange={(enabled) => patch({ enabled })}
              tone="ok"
            />
          </div>

          {gear.enabled && (
            <>
              <div className="flex items-center gap-2 min-h-6 pt-2">
                <span className="shrink-0 micro-label">Agregar trigger</span>
                <div className="min-w-0 flex-1">
                  <DarkSelect
                    size="sm"
                    variant="keycap"
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
                <p className="py-2 text-center text-caption text-muted">
                  Agrega una tecla del spammer y define su equipo ATK / DEF.
                </p>
              ) : (
                <div className="space-y-2">
                  {gear.rules.map((rule) => (
                    <div key={rule.trigger} className="space-y-1 py-2">
                      <div className="flex items-center justify-between gap-2">
                        <span className="micro-label">
                          Trigger{' '}
                          <span className="ml-1 font-mono px-1.5 py-0.5 font-semibold text-ink">
                            {rule.trigger}
                          </span>
                        </span>
                        <button
                          type="button"
                          disabled={disabled}
                          onClick={() =>
                            onChange(removeGearRule(gear, rule.trigger))
                          }
                          className="rounded-inline p-0.5 text-muted transition-colors hover:bg-panel-raised hover:text-bad idle-control"
                          aria-label={`Eliminar regla ${rule.trigger}`}
                        >
                          <X className="h-3 w-3" />
                        </button>
                      </div>
                      <GearKeySet
                        label="ATK"
                        tone="selection"
                        keys={rule.atkKeys}
                        disabled={disabled}
                        onToggle={(key) =>
                          toggleRuleKey(rule.trigger, 'atkKeys', key)
                        }
                      />
                      <GearKeySet
                        label="DEF"
                        tone="info"
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
                <span className="micro-label shrink-0">Switch</span>
                <Input
                  variant="inline"
                  type="range"
                  min={GEAR_SWITCH_MIN_DELAY_MS}
                  max={GEAR_SWITCH_MAX_DELAY_MS}
                  step={5}
                  disabled={disabled}
                  value={gear.switchDelayMs}
                  onChange={(event) =>
                    patch({ switchDelayMs: Number(event.target.value) })
                  }
                  className="font-mono flex-1 idle-control"
                />
                <span className="text-caption font-mono text-muted w-10 text-right shrink-0">
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
