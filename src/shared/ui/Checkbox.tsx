import { Circle } from 'lucide-react'
import type { Tone } from './types'

interface CheckboxProps {
  checked: boolean
  onChange: (checked: boolean) => void
  disabled?: boolean
  label: string
  tone?: Tone
}

const CHECKED_CLASSES: Record<Tone, string> = {
  warn: 'border-warn bg-field text-warn',
  ok: 'border-ok bg-field text-ok',
  bad: 'border-bad bg-field text-bad',
  info: 'border-info bg-field text-info',
  neutral: 'border-muted bg-field text-muted',
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
      className={`idle-control flex h-4 w-4 shrink-0 items-center justify-center rounded-pill border-[1.5px] transition-colors duration-120
        focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent/50 disabled:cursor-not-allowed
        ${
          checked
            ? tone
              ? CHECKED_CLASSES[tone]
              : 'border-accent bg-field text-accent'
            : 'border-outline bg-field text-transparent hover:border-muted'
        }`}
    >
      <Circle
        className="h-2 w-2"
        strokeWidth={0}
        fill="currentColor"
        aria-hidden
      />
    </button>
  )
}
