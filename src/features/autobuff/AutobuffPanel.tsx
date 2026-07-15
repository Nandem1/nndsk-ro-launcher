import { useCallback } from 'react'
import { Sparkles, X } from 'lucide-react'
import { Panel, resolveToolTone } from '../../shared/ui/Panel'
import { ToggleSwitch } from '../../shared/ui/ToggleSwitch'
import { DarkSelect } from '../../shared/ui/DarkSelect'
import { Button } from '../../shared/ui/Button'
import { Checkbox } from '../../shared/ui/Checkbox'
import { SPAMMER_KEYS } from '../../shared/constants'
import { useSelectedServer } from '../servers/useSelectedServer'
import { useLauncherStore } from '../launcher/launcher.store'
import { useUiModeStore } from '../../app/uiMode.store'
import { useAutobuff } from './useAutobuff'
import type { AutobuffRule } from '../../shared/types'

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
  const id = crypto.randomUUID()
  return {
    id,
    label: 'Nuevo buff',
    statusId: 1,
    key: 'F1',
    cooldownMs: 1000,
    priority: 100,
    enabled: false,
  }
}

export function AutobuffPanel() {
  const server = useSelectedServer()
  const { config, status, busy, isRunning, error, setEnabled, updateField } =
    useAutobuff(server)
  const launching = useLauncherStore((s) => s.status === 'launching')
  const hero = useUiModeStore((s) => s.mode === 'ingame')
  const available = isRunning && !!server
  const tone = resolveToolTone(
    available,
    config.enabled && status.active,
    !!error,
  )
  const replaceRules = useCallback(
    (rules: AutobuffRule[]) => void updateField({ rules }),
    [updateField],
  )
  const hasStatusId = (statusId: number, exceptId?: string) =>
    config.rules.some(
      (rule) => rule.id !== exceptId && rule.statusId === statusId,
    )
  const addPreset = (preset: Omit<AutobuffRule, 'id'>) => {
    if (hasStatusId(preset.statusId)) return
    replaceRules([...config.rules, { ...preset, id: crypto.randomUUID() }])
  }
  const updateRule = (id: string, patch: Partial<AutobuffRule>) => {
    const current = config.rules.find((rule) => rule.id === id)
    const nextStatusId = patch.statusId ?? current?.statusId
    if (!nextStatusId || nextStatusId < 0) return
    if (nextStatusId && hasStatusId(nextStatusId, id)) return
    replaceRules(
      config.rules.map((rule) =>
        rule.id === id ? { ...rule, ...patch } : rule,
      ),
    )
  }

  return (
    <Panel
      title="AutoBuff"
      compact
      hero={hero}
      tone={tone}
      className="h-full w-full"
      leading={
        <Sparkles className="w-3 h-3 text-zinc-600 shrink-0" aria-hidden />
      }
    >
      <div className="flex min-h-0 flex-1 flex-col gap-2">
        <div className="flex shrink-0 items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-semibold text-zinc-100">
              {status.lastAppliedRule ?? 'Sin buffs aplicados'}
            </p>
            <p
              className={`text-[10px] ${launching ? 'text-zinc-500 animate-pulse-dot' : 'text-zinc-600'}`}
            >
              {!server
                ? 'Selecciona un servidor'
                : launching
                  ? 'Iniciando juego...'
                  : !isRunning
                    ? 'Inicia el juego'
                    : `${status.activeStatuses} estados detectados`}
            </p>
          </div>
          <ToggleSwitch
            checked={
              config.enabled &&
              available &&
              config.rules.some((rule) => rule.enabled)
            }
            disabled={
              !available || busy || !config.rules.some((rule) => rule.enabled)
            }
            onChange={(enabled) => void setEnabled(enabled)}
            tone="emerald"
          />
        </div>

        <div className="shrink-0 space-y-1.5">
          {PRESET_GROUPS.map((group) => (
            <div key={group.label}>
              <p className="mb-0.5 text-[10px] uppercase tracking-wide text-zinc-600">
                {group.label}
              </p>
              <div className="flex flex-wrap gap-1">
                {group.presets.map((preset) => (
                  <Button
                    key={preset.label}
                    variant="secondary"
                    size="xs"
                    disabled={!server || busy || hasStatusId(preset.statusId)}
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
            disabled={!server || busy || hasStatusId(1)}
            onClick={() => replaceRules([...config.rules, makeRule()])}
          >
            + Manual
          </Button>
        </div>

        <div className="min-h-14 flex-1 space-y-1 overflow-y-auto rounded-lg border border-zinc-800/60 bg-zinc-950/40 p-1.5">
          {config.rules.length === 0 ? (
            <p className="px-1 py-2 text-[10px] text-zinc-600">
              Añade un preset o una regla manual.
            </p>
          ) : (
            <>
              <div className="grid grid-cols-[16px_minmax(0,1fr)_68px_84px_64px_20px] items-center gap-1.5 px-2 text-[9px] uppercase tracking-wide text-zinc-600">
                <span />
                <span>Buff</span>
                <span className="text-center">Tecla</span>
                <span className="text-center">Cooldown</span>
                <span className="text-center">Prioridad</span>
                <span />
              </div>
              {config.rules.map((rule) => (
                <div
                  key={rule.id}
                  className="group grid grid-cols-[16px_minmax(0,1fr)_68px_84px_64px_20px] items-center gap-1.5 rounded-md border border-white/[0.04] bg-zinc-950/35 px-2 py-1 transition-colors hover:border-white/[0.07] hover:bg-zinc-900/45"
                >
                  <Checkbox
                    checked={rule.enabled}
                    disabled={!server || busy}
                    onChange={(enabled) => updateRule(rule.id, { enabled })}
                    label={`${rule.enabled ? 'Desactivar' : 'Activar'} ${rule.label}`}
                  />
                  <input
                    value={rule.label}
                    disabled={!server || busy}
                    onChange={(e) =>
                      updateRule(rule.id, { label: e.target.value })
                    }
                    className="min-w-0 rounded border border-transparent bg-transparent px-1.5 py-1 text-[10px] font-medium text-zinc-300 outline-none transition-colors hover:bg-white/[0.025] focus:border-amber-500/25 focus:bg-zinc-950/60 focus:ring-1 focus:ring-amber-500/10"
                  />
                  <DarkSelect
                    compact
                    keycap
                    value={rule.key}
                    disabled={!server || busy}
                    onChange={(key) => updateRule(rule.id, { key })}
                    options={KEY_OPTIONS}
                  />
                  <div className="flex items-center gap-0.5">
                    <input
                      type="number"
                      min={0}
                      step={100}
                      value={rule.cooldownMs}
                      disabled={!server || busy}
                      onChange={(e) =>
                        updateRule(rule.id, {
                          cooldownMs: Math.max(0, Number(e.target.value) || 0),
                        })
                      }
                      className="min-w-0 flex-1 rounded border border-transparent bg-zinc-950/40 px-1 py-1 text-right text-[10px] text-zinc-300 outline-none transition-colors hover:bg-white/[0.025] focus:border-amber-500/25 focus:bg-zinc-950/60 focus:ring-1 focus:ring-amber-500/10 [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none"
                      aria-label={`Cooldown de ${rule.label} en ms`}
                    />
                    <span className="text-[9px] text-zinc-600">ms</span>
                  </div>
                  <input
                    type="number"
                    min={0}
                    step={1}
                    value={rule.priority}
                    disabled={!server || busy}
                    onChange={(e) =>
                      updateRule(rule.id, {
                        priority: Math.max(0, Number(e.target.value) || 0),
                      })
                    }
                    className="min-w-0 rounded border border-transparent bg-zinc-950/40 px-1 py-1 text-center text-[10px] text-zinc-300 outline-none transition-colors hover:bg-white/[0.025] focus:border-amber-500/25 focus:bg-zinc-950/60 focus:ring-1 focus:ring-amber-500/10 [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none"
                    aria-label={`Prioridad de ${rule.label}`}
                  />
                  <button
                    type="button"
                    disabled={!server || busy}
                    onClick={() =>
                      replaceRules(
                        config.rules.filter((item) => item.id !== rule.id),
                      )
                    }
                    className="flex h-5 w-5 items-center justify-center rounded text-zinc-700 opacity-50 transition-[color,background-color,opacity] hover:bg-red-500/10 hover:text-red-400 hover:opacity-100 group-hover:opacity-80 disabled:opacity-30"
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
        <p className="shrink-0 text-[10px] leading-snug text-zinc-600">
          Activa cada buff y asigna la tecla donde lo tienes configurado en el
          juego.
        </p>
        <p className="shrink-0 text-[10px] leading-snug min-h-[calc(1em*1.375)]">
          {error && available ? (
            <span className="text-red-400/90">{error}</span>
          ) : null}
        </p>
      </div>
    </Panel>
  )
}
