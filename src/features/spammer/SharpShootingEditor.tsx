import { ChevronDown, Crosshair } from 'lucide-react'
import { useState } from 'react'
import type { ShiftModeConfig } from '../../shared/types'
import { ToggleSwitch } from '../../shared/ui/ToggleSwitch'
import { toggleShiftModeTrigger } from './spammer.logic'

interface SharpShootingEditorProps {
  spammerKeys: string[]
  shiftMode: ShiftModeConfig
  shiftActive?: boolean
  disabled: boolean
  onChange: (shiftMode: ShiftModeConfig) => void
}

export function SharpShootingEditor({
  spammerKeys,
  shiftMode,
  shiftActive,
  disabled,
  onChange,
}: SharpShootingEditorProps) {
  const [open, setOpen] = useState(false)

  const setEnabled = (enabled: boolean) => {
    const triggerKeys =
      enabled && shiftMode.triggerKeys.length === 0 && spammerKeys[0]
        ? [spammerKeys[0]]
        : shiftMode.triggerKeys
    onChange({ ...shiftMode, enabled, triggerKeys })
  }

  return (
    <div className="rounded-lg bg-zinc-950/40 border border-zinc-800/60">
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        className="w-full flex items-center justify-between gap-2 px-2.5 py-2 text-left"
      >
        <span className="flex items-center gap-1.5 text-[10px] uppercase tracking-wide text-zinc-500">
          <Crosshair className="w-3 h-3 shrink-0" aria-hidden />
          Sharp Shooting / Focused Arrow Strike
          {shiftMode.enabled && (
            <span className="rounded bg-violet-500/15 px-1 text-[9px] font-semibold text-violet-300 normal-case tracking-normal">
              {shiftActive ? 'SHIFT' : 'on'}
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
          <div className="flex items-start justify-between gap-2">
            <p className="text-[10px] text-zinc-600 leading-snug">
              Mantiene Shift durante skill + click. AutoPot usa Shift con sus
              teclas configuradas mientras el modo está activo.
            </p>
            <ToggleSwitch
              checked={shiftMode.enabled}
              disabled={disabled || spammerKeys.length === 0}
              onChange={setEnabled}
              tone="amber"
            />
          </div>

          {shiftMode.enabled && (
            <div className="space-y-1.5 border-t border-zinc-800/60 pt-2">
              <span className="text-[10px] uppercase tracking-wide text-zinc-600">
                Triggers con Shift
              </span>
              {spammerKeys.length === 0 ? (
                <p className="text-[10px] text-zinc-600">
                  Selecciona una tecla en el spammer.
                </p>
              ) : (
                <div className="flex flex-wrap gap-1">
                  {spammerKeys.map((key) => {
                    const selected = shiftMode.triggerKeys.includes(key)
                    return (
                      <button
                        key={key}
                        type="button"
                        disabled={disabled}
                        aria-pressed={selected}
                        aria-label={`Usar ${key} con Shift`}
                        onClick={() =>
                          onChange(toggleShiftModeTrigger(shiftMode, key))
                        }
                        className={`min-w-8 rounded-md border px-2 py-1 text-[10px] font-semibold transition-colors disabled:opacity-40 ${
                          selected
                            ? 'border-violet-500/70 bg-violet-500/15 text-violet-200'
                            : 'border-zinc-800 bg-zinc-900/40 text-zinc-500 hover:border-zinc-700 hover:text-zinc-300'
                        }`}
                      >
                        {key}
                      </button>
                    )
                  })}
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
