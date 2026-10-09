interface ToggleSwitchProps {
  checked: boolean
  disabled?: boolean
  onChange: (checked: boolean) => void
  tone?: 'emerald' | 'amber'
}

export function ToggleSwitch({
  checked,
  disabled = false,
  onChange,
  tone = 'emerald',
}: ToggleSwitchProps) {
  const onClass =
    tone === 'emerald'
      ? 'bg-ok/80 border-ok-bright/50 shadow-glow-ok'
      : 'bg-accent/80 border-accent-bright/50 shadow-glow-warn'

  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative w-9 h-5 rounded-pill border transition-[background-color,border-color,box-shadow] duration-200 shrink-0 disabled:opacity-50 disabled:cursor-not-allowed ${
        checked ? onClass : 'bg-panel-raised border-line/80'
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
