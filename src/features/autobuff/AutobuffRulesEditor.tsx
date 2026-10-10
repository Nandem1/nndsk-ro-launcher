import { X } from 'lucide-react'
import { SPAMMER_KEYS } from '../../shared/constants'
import type { AutobuffRule } from '../../shared/types'
import { Button } from '../../shared/ui/Button'
import { Checkbox } from '../../shared/ui/Checkbox'
import { DarkSelect } from '../../shared/ui/DarkSelect'

const KEY_OPTIONS = SPAMMER_KEYS.map((key) => ({ value: key, label: key }))

const PRESET_GROUPS: { label: string; presets: Omit<AutobuffRule, 'id'>[] }[] =
  [
    {
      label: 'Potions',
      presets: [
        {
          label: 'Concentration Potion',
          statusId: 37,
          key: 'F1',
          cooldownMs: 1000,
          priority: 10,
          enabled: true,
        },
        {
          label: 'Awakening Potion',
          statusId: 38,
          key: 'F2',
          cooldownMs: 1000,
          priority: 20,
          enabled: true,
        },
        {
          label: 'Berserk Potion',
          statusId: 39,
          key: 'F3',
          cooldownMs: 1000,
          priority: 30,
          enabled: true,
        },
      ],
    },
    {
      label: 'Resistance',
      presets: [
        {
          label: 'Fireproof Potion',
          statusId: 910,
          key: 'F4',
          cooldownMs: 1000,
          priority: 40,
          enabled: true,
        },
        {
          label: 'Waterproof Potion',
          statusId: 908,
          key: 'F5',
          cooldownMs: 1000,
          priority: 50,
          enabled: true,
        },
        {
          label: 'Windproof Potion',
          statusId: 911,
          key: 'F6',
          cooldownMs: 1000,
          priority: 60,
          enabled: true,
        },
        {
          label: 'Earthproof Potion',
          statusId: 909,
          key: 'F7',
          cooldownMs: 1000,
          priority: 70,
          enabled: true,
        },
      ],
    },
    {
      label: 'Boxes',
      presets: [
        {
          label: 'Box of Gloom',
          statusId: 3,
          key: 'F8',
          cooldownMs: 1000,
          priority: 80,
          enabled: true,
        },
        {
          label: 'Sunlight Box',
          statusId: 184,
          key: 'F9',
          cooldownMs: 1000,
          priority: 90,
          enabled: true,
        },
      ],
    },
  ]

function makeRule(): AutobuffRule {
  return {
    id: crypto.randomUUID(),
    label: 'Nuevo buff',
    statusId: 1,
    key: 'F1',
    cooldownMs: 1000,
    priority: 100,
    enabled: false,
  }
}

interface AutobuffRulesEditorProps {
  rules: AutobuffRule[]
  disabled: boolean
  onChange: (rules: AutobuffRule[]) => void
}

export function AutobuffRulesEditor({
  rules,
  disabled,
  onChange,
}: AutobuffRulesEditorProps) {
  const hasStatusId = (statusId: number, exceptId?: string) =>
    rules.some((rule) => rule.id !== exceptId && rule.statusId === statusId)
  const addPreset = (preset: Omit<AutobuffRule, 'id'>) => {
    if (!hasStatusId(preset.statusId)) {
      onChange([...rules, { ...preset, id: crypto.randomUUID() }])
    }
  }
  const updateRule = (id: string, patch: Partial<AutobuffRule>) => {
    const current = rules.find((rule) => rule.id === id)
    const nextStatusId = patch.statusId ?? current?.statusId
    if (!nextStatusId || nextStatusId < 0 || hasStatusId(nextStatusId, id))
      return
    onChange(
      rules.map((rule) => (rule.id === id ? { ...rule, ...patch } : rule)),
    )
  }

  return (
    <>
      <div className="shrink-0 space-y-1.5">
        {PRESET_GROUPS.map((group) => (
          <div key={group.label}>
            <p className="mb-0.5 text-caption uppercase tracking-wide text-line-strong">
              {group.label}
            </p>
            <div className="flex flex-wrap gap-1">
              {group.presets.map((preset) => (
                <Button
                  key={preset.label}
                  variant="secondary"
                  size="xs"
                  disabled={disabled || hasStatusId(preset.statusId)}
                  onClick={() => addPreset(preset)}
                >
                  + {preset.label}
                </Button>
              ))}
            </div>
          </div>
        ))}
        <Button
          variant="primary"
          size="xs"
          disabled={disabled || hasStatusId(1)}
          onClick={() => onChange([...rules, makeRule()])}
        >
          + Manual
        </Button>
      </div>

      <div className="min-h-14 flex-1 space-y-1 overflow-y-auto rounded-control border border-panel-raised/60 bg-surface/40 p-1.5">
        {rules.length === 0 ? (
          <p className="px-1 py-2 text-caption text-line-strong">
            Añade un preset o una regla manual.
          </p>
        ) : (
          <>
            <div className="grid grid-cols-[16px_minmax(0,1fr)_68px_84px_64px_20px] items-center gap-1.5 px-2 text-micro uppercase tracking-wide text-line-strong">
              <span />
              <span>Buff</span>
              <span className="text-center">Tecla</span>
              <span className="text-center">Cooldown</span>
              <span className="text-center">Prioridad</span>
              <span />
            </div>
            {rules.map((rule) => (
              <div
                key={rule.id}
                className="group grid grid-cols-[16px_minmax(0,1fr)_68px_84px_64px_20px] items-center gap-1.5 rounded-control-compact border border-overlay-light/[0.04] bg-surface/35 px-2 py-1 transition-colors hover:border-overlay-light/[0.07] hover:bg-panel/45"
              >
                <Checkbox
                  checked={rule.enabled}
                  disabled={disabled}
                  onChange={(enabled) => updateRule(rule.id, { enabled })}
                  label={`${rule.enabled ? 'Desactivar' : 'Activar'} ${rule.label}`}
                />
                <input
                  value={rule.label}
                  disabled={disabled}
                  onChange={(event) =>
                    updateRule(rule.id, { label: event.target.value })
                  }
                  className="min-w-0 rounded-inline border border-transparent bg-transparent px-1.5 py-1 text-caption font-medium text-ink-dim outline-none transition-colors hover:bg-overlay-light/[0.025] focus:border-accent/25 focus:bg-surface/60 focus:ring-1 focus:ring-accent/10"
                />
                <DarkSelect
                  size="sm"
                  variant="keycap"
                  value={rule.key}
                  disabled={disabled}
                  onChange={(key) => updateRule(rule.id, { key })}
                  options={KEY_OPTIONS}
                />
                <div className="flex items-center gap-0.5">
                  <input
                    type="number"
                    min={0}
                    step={100}
                    value={rule.cooldownMs}
                    disabled={disabled}
                    onChange={(event) =>
                      updateRule(rule.id, {
                        cooldownMs: Math.max(
                          0,
                          Number(event.target.value) || 0,
                        ),
                      })
                    }
                    className="min-w-0 flex-1 rounded-inline border border-transparent bg-surface/40 px-1 py-1 text-right text-caption text-ink-dim outline-none transition-colors hover:bg-overlay-light/[0.025] focus:border-accent/25 focus:bg-surface/60 focus:ring-1 focus:ring-accent/10 [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none"
                    aria-label={`Cooldown de ${rule.label} en ms`}
                  />
                  <span className="text-micro text-line-strong">ms</span>
                </div>
                <input
                  type="number"
                  min={0}
                  step={1}
                  value={rule.priority}
                  disabled={disabled}
                  onChange={(event) =>
                    updateRule(rule.id, {
                      priority: Math.max(0, Number(event.target.value) || 0),
                    })
                  }
                  className="min-w-0 rounded-inline border border-transparent bg-surface/40 px-1 py-1 text-center text-caption text-ink-dim outline-none transition-colors hover:bg-overlay-light/[0.025] focus:border-accent/25 focus:bg-surface/60 focus:ring-1 focus:ring-accent/10 [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none"
                  aria-label={`Prioridad de ${rule.label}`}
                />
                <button
                  type="button"
                  disabled={disabled}
                  onClick={() =>
                    onChange(rules.filter((item) => item.id !== rule.id))
                  }
                  className="flex h-5 w-5 items-center justify-center rounded-inline text-line opacity-50 transition-[color,background-color,opacity] hover:bg-bad/10 hover:text-bad-bright hover:opacity-100 group-hover:opacity-80 disabled:opacity-30"
                  title="Eliminar regla"
                  aria-label={`Eliminar ${rule.label}`}
                >
                  <X className="w-3 h-3" />
                </button>
              </div>
            ))}
          </>
        )}
      </div>
    </>
  )
}
