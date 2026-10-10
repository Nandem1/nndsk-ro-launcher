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
  warn: 'border-warn/50 bg-field text-warn',
  ok: 'border-ok/50 bg-field text-ok',
  bad: 'border-bad/50 bg-field text-bad',
  info: 'border-info/50 bg-field text-info',
  neutral: 'border-line-strong bg-field text-ink',
}

export function Checkbox({
  checked,
  onChange,
  disabled = false,
  label,
  tone,
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
      className={`flex h-4 w-4 shrink-0 items-center justify-center rounded-inline border transition-colors duration-150
        focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-40
        ${
          checked
            ? tone
              ? CHECKED_CLASSES[tone]
              : 'border-accent bg-field text-accent'
            : 'border-line-strong bg-field text-transparent hover:border-muted'
        }`}
    >
      <Check className="h-3 w-3" strokeWidth={3} aria-hidden />
    </button>
  )
}
