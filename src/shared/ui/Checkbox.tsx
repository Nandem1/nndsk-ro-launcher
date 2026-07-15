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
      className={`flex h-4 w-4 shrink-0 items-center justify-center rounded border transition-[background-color,border-color,box-shadow,color]
        focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/30 disabled:cursor-not-allowed disabled:opacity-40
        ${
          checked
            ? 'border-amber-500/55 bg-amber-500/12 text-amber-300 shadow-[inset_0_0_0_1px_rgb(245_158_11_/_0.06)]'
            : 'border-zinc-700/80 bg-zinc-950/50 text-transparent hover:border-zinc-500'
        }`}
    >
      <Check className="h-3 w-3" strokeWidth={3} aria-hidden />
    </button>
  )
}
