import { Check } from 'lucide-react'
import type { Tone } from './types'

interface CheckboxProps {
  checked: boolean
  onChange: (checked: boolean) => void
  disabled?: boolean
  label: string
  tone?: Tone
}

const CHECKED_CLASSES: Record<Tone, string> = {
  warn: 'border-accent/55 bg-accent/12 text-accent-light shadow-check-warn',
  ok: 'border-ok/30 bg-ok/10 text-ok-soft',
  bad: 'border-bad/30 bg-bad/10 text-bad-soft',
  info: 'border-info/70 bg-info/15 text-info-soft',
  neutral: 'border-line/80 bg-surface/50 text-ink-dim',
}

export function Checkbox({
  checked,
  onChange,
  disabled = false,
  label,
  tone = 'warn',
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
            ? CHECKED_CLASSES[tone]
            : 'border-line/80 bg-surface/50 text-transparent hover:border-muted'
        }`}
    >
      <Check className="h-3 w-3" strokeWidth={3} aria-hidden />
    </button>
  )
}
