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
      className={`idle-control relative w-[34px] h-[18px] rounded-pill border transition-colors duration-150 shrink-0 disabled:cursor-not-allowed ${
        checked ? 'bg-ok border-ok' : 'bg-field border-muted'
      }`}
    >
      <span
        className={`absolute top-px w-3.5 h-3.5 rounded-pill transition-colors duration-150 ${
          checked ? 'left-[17px] bg-on-accent' : 'left-px bg-muted'
        }`}
      />
    </button>
  )
}
