import { useCallback } from 'react'
import { Panel, type PanelTone } from '../../shared/ui/Panel'
import { ToggleSwitch } from '../../shared/ui/ToggleSwitch'
import { DarkSelect } from '../../shared/ui/DarkSelect'
import { SPAMMER_KEYS } from '../../shared/constants'
import { useSelectedServer } from '../servers/useSelectedServer'
import { useAutobuff } from './useAutobuff'
import type { AutobuffRule } from '../../shared/types'

const KEY_OPTIONS = SPAMMER_KEYS.map((key) => ({ value: key, label: key }))

const PRESET_GROUPS: { label: string; presets: Omit<AutobuffRule, 'id'>[] }[] = [
  {
    label: 'Potions',
    presets: [
      { label: 'Concentration Potion', statusId: 37, key: 'F1', cooldownMs: 1000, priority: 10, enabled: true },
      { label: 'Awakening Potion', statusId: 38, key: 'F2', cooldownMs: 1000, priority: 20, enabled: true },
      { label: 'Berserk Potion', statusId: 39, key: 'F3', cooldownMs: 1000, priority: 30, enabled: true },
    ],
  },
  {
    label: 'Resistance',
    presets: [
      { label: 'Fireproof Potion', statusId: 910, key: 'F4', cooldownMs: 1000, priority: 40, enabled: true },
      { label: 'Waterproof Potion', statusId: 908, key: 'F5', cooldownMs: 1000, priority: 50, enabled: true },
      { label: 'Windproof Potion', statusId: 911, key: 'F6', cooldownMs: 1000, priority: 60, enabled: true },
      { label: 'Earthproof Potion', statusId: 909, key: 'F7', cooldownMs: 1000, priority: 70, enabled: true },
    ],
  },
  {
    label: 'Boxes',
    presets: [
      { label: 'Box of Gloom', statusId: 3, key: 'F8', cooldownMs: 1000, priority: 80, enabled: true },
      { label: 'Sunlight Box', statusId: 184, key: 'F9', cooldownMs: 1000, priority: 90, enabled: true },
    ],
  },
]

function makeRule(): AutobuffRule {
  const id = crypto.randomUUID()
  return { id, label: 'Nuevo buff', statusId: 1, key: 'F1', cooldownMs: 1000, priority: 100, enabled: false }
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

export function AutobuffPanel() {
  const server = useSelectedServer()
  const { config, status, busy, isRunning, error, setEnabled, updateField } = useAutobuff(server)
  const available = isRunning && !!server
  const tone = resolveTone(available, config.enabled, status.active, !!error)
  const replaceRules = useCallback((rules: AutobuffRule[]) => void updateField({ rules }), [updateField])
  const hasStatusId = (statusId: number, exceptId?: string) => config.rules.some((rule) => rule.id !== exceptId && rule.statusId === statusId)
  const addPreset = (preset: Omit<AutobuffRule, 'id'>) => {
    if (hasStatusId(preset.statusId)) return
    replaceRules([...config.rules, { ...preset, id: crypto.randomUUID() }])
  }
  const updateRule = (id: string, patch: Partial<AutobuffRule>) => {
    const current = config.rules.find((rule) => rule.id === id)
    const nextStatusId = patch.statusId ?? current?.statusId
    if (!nextStatusId || nextStatusId < 0) return
    if (nextStatusId && hasStatusId(nextStatusId, id)) return
    replaceRules(config.rules.map((rule) => rule.id === id ? { ...rule, ...patch } : rule))
  }

  return (
    <Panel title="AutoBuff" compact tone={tone} className="h-full">
      <div className="space-y-2">
        <div className="flex items-start justify-between gap-2">
          <div>
            <p className="text-sm font-semibold text-zinc-100">{status.lastAppliedRule ?? 'Sin buffs aplicados'}</p>
            <p className="text-[10px] text-zinc-600">{available ? `${status.activeStatuses} estados detectados` : server ? 'Inicia el juego' : 'Selecciona un servidor'}</p>
          </div>
          <ToggleSwitch checked={config.enabled && available && config.rules.some((rule) => rule.enabled)} disabled={!available || busy || !config.rules.some((rule) => rule.enabled)} onChange={(enabled) => void setEnabled(enabled)} tone="emerald" />
        </div>

        <div className="space-y-1.5">
          {PRESET_GROUPS.map((group) => (
            <div key={group.label}>
              <p className="mb-0.5 text-[9px] uppercase tracking-wide text-zinc-600">{group.label}</p>
              <div className="flex flex-wrap gap-1">
                {group.presets.map((preset) => <button key={preset.label} type="button" disabled={!server || busy || hasStatusId(preset.statusId)} onClick={() => addPreset(preset)} className="rounded border border-zinc-700/80 px-1.5 py-1 text-[10px] text-zinc-400 hover:text-amber-200 disabled:opacity-40">+ {preset.label}</button>)}
              </div>
            </div>
          ))}
          <button type="button" disabled={!server || busy || hasStatusId(1)} onClick={() => replaceRules([...config.rules, makeRule()])} className="rounded border border-amber-500/40 px-1.5 py-1 text-[10px] text-amber-300 disabled:opacity-40">+ Manual</button>
        </div>

        <div className="max-h-36 space-y-1 overflow-y-auto rounded-lg border border-zinc-800/60 bg-zinc-950/30 p-1.5">
          {config.rules.length === 0 ? <p className="px-1 py-2 text-[10px] text-zinc-600">Añade un preset o una regla manual.</p> : config.rules.map((rule) => (
            <div key={rule.id} className="grid grid-cols-[18px_1fr_45px_34px_18px] items-center gap-1">
              <input type="checkbox" checked={rule.enabled} disabled={!server || busy} onChange={(e) => updateRule(rule.id, { enabled: e.target.checked })} />
              <input value={rule.label} disabled={!server || busy} onChange={(e) => updateRule(rule.id, { label: e.target.value })} className="min-w-0 rounded border border-zinc-800 bg-zinc-950 px-1 py-1 text-[10px] text-zinc-300" />
              <input type="number" value={rule.statusId} disabled={!server || busy} onChange={(e) => updateRule(rule.id, { statusId: Number(e.target.value) || 0 })} title="Status ID" className="w-full rounded border border-zinc-800 bg-zinc-950 px-1 py-1 text-[10px] text-zinc-300" />
              <DarkSelect value={rule.key} disabled={!server || busy} onChange={(key) => updateRule(rule.id, { key })} options={KEY_OPTIONS} />
              <button type="button" disabled={!server || busy} onClick={() => replaceRules(config.rules.filter((item) => item.id !== rule.id))} className="text-xs text-zinc-600 hover:text-red-400 disabled:opacity-40">×</button>
            </div>
          ))}
        </div>
        <p className="text-[10px] text-zinc-600">Un status ID solo puede tener una tecla. Escanea HP + 0x474 cada {config.delayMs}ms.</p>
        <p className="min-h-[calc(1em*1.375)] text-[10px] text-red-400/90">{error && available ? error : ''}</p>
      </div>
    </Panel>
  )
}
