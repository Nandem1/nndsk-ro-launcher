import { memo, useMemo } from 'react'
import {
  SPAMMER_FUNCTION_KEYS,
  SPAMMER_LETTER_KEY_ROWS,
  SPAMMER_NUMBER_KEYS,
} from '../../shared/constants'
import type { SpammerConfig } from '../../shared/types'
import { formatSpammerKeys, toggleSpammerKey } from './spammer.logic'

const KeyChip = memo(function KeyChip({
  label,
  active,
  disabled,
  onToggle,
}: {
  label: string
  active: boolean
  disabled: boolean
  onToggle: () => void
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onToggle}
      className={`min-w-0 flex-1 px-1 py-1 rounded-md text-[10px] font-semibold border transition-colors motion-safe:active:scale-[0.97] disabled:opacity-40 ${
        active
          ? 'border-amber-500/70 bg-amber-500/15 text-amber-200'
          : 'border-zinc-800/80 bg-zinc-950/50 text-zinc-600 hover:text-zinc-400'
      }`}
    >
      {label}
    </button>
  )
})

interface SpammerKeyboardProps {
  config: SpammerConfig
  armed: boolean
  available: boolean
  disabled: boolean
  onKeysChange: (keys: string[]) => void
}

export function SpammerKeyboard({
  config,
  armed,
  available,
  disabled,
  onKeysChange,
}: SpammerKeyboardProps) {
  const selected = useMemo(() => new Set(config.keys), [config.keys])
  const label = formatSpammerKeys(config.keys)
  const toggle = (key: string) =>
    onKeysChange(toggleSpammerKey(config, key).keys)

  return (
    <div className="space-y-1.5 rounded-lg bg-zinc-950/40 border border-zinc-800/60 px-2.5 py-2">
      <div className="flex justify-between text-[10px]">
        <span className="text-zinc-600 uppercase tracking-wide">Teclas</span>
        <span
          className={
            available && armed
              ? 'text-amber-400/90 font-medium truncate ml-2'
              : 'text-zinc-700 truncate ml-2'
          }
        >
          {label}
        </span>
      </div>
      <div className="space-y-1">
        {[SPAMMER_FUNCTION_KEYS, SPAMMER_NUMBER_KEYS].map((row, rowIndex) => (
          <div key={rowIndex} className="flex gap-1">
            {row.map((key) => (
              <KeyChip
                key={key}
                label={key}
                active={selected.has(key)}
                disabled={disabled}
                onToggle={() => toggle(key)}
              />
            ))}
          </div>
        ))}
        <div className="space-y-1 pt-0.5">
          {SPAMMER_LETTER_KEY_ROWS.map((row, rowIndex) => (
            <div
              key={rowIndex}
              className={`flex gap-1 ${
                rowIndex === 1 ? 'px-[5%]' : rowIndex === 2 ? 'px-[15%]' : ''
              }`}
            >
              {row.map((key) => (
                <KeyChip
                  key={key}
                  label={key}
                  active={selected.has(key)}
                  disabled={disabled}
                  onToggle={() => toggle(key)}
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
  )
}
