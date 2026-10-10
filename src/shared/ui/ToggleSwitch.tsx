import type { Tone } from './types'

interface ToggleSwitchProps {
  checked: boolean
  disabled?: boolean
  onChange: (checked: boolean) => void
  tone?: Tone
}

export function ToggleSwitch({
  checked,
  disabled = false,
  onChange,
}: ToggleSwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`idle-control relative w-9 h-5 rounded-pill transition-colors duration-120 shrink-0 disabled:cursor-not-allowed ${
        checked ? 'bg-ok' : 'bg-track'
      }`}
    >
      <span
        className={`absolute top-0.5 w-4 h-4 rounded-pill transition-colors duration-120 ${
          checked ? 'left-[18px] bg-surface' : 'left-0.5 bg-muted'
        }`}
      />
    </button>
  )
}
