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
      className={`min-w-0 flex-1 h-7 p-0 rounded-control-compact font-mono text-key leading-[normal] transition-colors duration-120 idle-control ${
        active
          ? 'bg-accent text-on-accent disabled:text-on-accent font-medium'
          : 'bg-panel-raised text-muted font-normal hover:bg-line hover:text-ink'
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
    <div className="space-y-1 py-2">
      <div className="flex justify-between text-caption">
        <span className="micro-label">Teclas</span>
        <span
          className={
            available && armed
              ? 'font-mono text-ok font-medium break-words min-w-0 ml-2'
              : 'font-mono text-muted break-words min-w-0 ml-2'
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
      <p className="text-caption text-muted leading-snug">
        Skill en barra + target con click izquierdo
      </p>
    </div>
  )
}
