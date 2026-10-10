import type { Tone } from './types'

interface ToggleSwitchProps {
  checked: boolean
  disabled?: boolean
  onChange: (checked: boolean) => void
  tone?: Tone
}

const ON_CLASSES: Record<Tone, string> = {
  ok: 'bg-ok/80 border-ok-bright/50 shadow-glow-ok',
  warn: 'bg-accent/80 border-accent-bright/50 shadow-glow-warn',
  bad: 'bg-bad/10 border-bad/30 shadow-glow-bad',
  info: 'bg-info/15 border-info/70',
  neutral: 'bg-panel-raised border-line/80',
}

export function ToggleSwitch({
  checked,
  disabled = false,
  onChange,
  tone = 'ok',
}: ToggleSwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative w-9 h-5 rounded-pill border transition-[background-color,border-color,box-shadow] duration-200 shrink-0 disabled:opacity-50 disabled:cursor-not-allowed ${
        checked ? ON_CLASSES[tone] : 'bg-panel-raised border-line/80'
      }`}
    >
      <span
        className={`absolute top-0.5 left-0.5 w-3.5 h-3.5 rounded-pill bg-overlay-light shadow-control transition-transform duration-200 ease-spring ${
          checked ? 'translate-x-4' : 'translate-x-0'
        }`}
      />
    </button>
  )
}
