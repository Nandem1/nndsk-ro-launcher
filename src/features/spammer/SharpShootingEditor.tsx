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
    <div className="rounded-control bg-surface border border-line">
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        className="w-full flex items-center justify-between gap-2 px-2.5 py-2 text-left"
      >
        <span className="flex items-center gap-1.5 text-caption uppercase tracking-wide text-muted">
          <Crosshair className="w-3 h-3 shrink-0" aria-hidden />
          Sharp Shooting / Focused Arrow Strike
          {shiftMode.enabled && (
            <span className="rounded-inline bg-panel px-1 text-micro font-semibold text-special normal-case tracking-normal">
              {shiftActive ? 'SHIFT' : 'on'}
            </span>
          )}
        </span>
        <ChevronDown
          className={`w-3 h-3 text-muted transition-colors ${open ? 'rotate-180' : ''}`}
          aria-hidden
        />
      </button>

      {open && (
        <div className="space-y-2 px-2.5 pb-2.5">
          <div className="flex items-start justify-between gap-2">
            <p className="text-caption text-muted leading-snug">
              Mantiene Shift durante skill + click. AutoPot usa Shift con sus
              teclas configuradas mientras el modo está activo.
            </p>
            <ToggleSwitch
              checked={shiftMode.enabled}
              disabled={disabled || spammerKeys.length === 0}
              onChange={setEnabled}
              tone="ok"
            />
          </div>

          {shiftMode.enabled && (
            <div className="space-y-1.5 border-t border-line pt-2">
              <span className="text-caption uppercase tracking-wide text-muted">
                Triggers con Shift
              </span>
              {spammerKeys.length === 0 ? (
                <p className="text-caption text-muted">
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
                        className={`min-w-8 rounded-control-compact border px-2 py-1 text-caption font-semibold transition-colors disabled:opacity-40 ${
                          selected
                            ? 'border-special/70 bg-panel text-special'
                            : 'border-panel-raised bg-panel text-muted hover:border-line hover:text-ink'
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
