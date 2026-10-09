import { Check } from 'lucide-react'

interface CheckboxProps {
  checked: boolean
  onChange: (checked: boolean) => void
  disabled?: boolean
  label: string
}

export function Checkbox({
  checked,
  onChange,
  disabled = false,
  label,
}: CheckboxProps) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`flex h-4 w-4 shrink-0 items-center justify-center rounded-inline border transition-[background-color,border-color,box-shadow,color]
        focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/30 disabled:cursor-not-allowed disabled:opacity-40
        ${
          checked
            ? 'border-accent/55 bg-accent/12 text-accent-light shadow-check-warn'
            : 'border-line/80 bg-surface/50 text-transparent hover:border-muted'
        }`}
    >
      <Check className="h-3 w-3" strokeWidth={3} aria-hidden />
    </button>
  )
}
