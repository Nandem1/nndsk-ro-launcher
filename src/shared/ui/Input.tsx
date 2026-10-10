import {
  forwardRef,
  type ComponentPropsWithoutRef,
  type CSSProperties,
} from 'react'

interface InputProps extends ComponentPropsWithoutRef<'input'> {
  variant?: 'modal' | 'config' | 'inline'
}

const VARIANT_CLASSES = {
  inline:
    'rounded-control bg-field border border-line font-mono text-data text-ink placeholder:text-muted outline-none transition-colors duration-120 hover:border-line-strong',
  modal:
    'rounded-control border border-line bg-field px-3 py-2 font-mono text-data text-ink placeholder:text-muted outline-none transition-colors duration-120 hover:border-line-strong',
  config:
    'bg-field border border-line rounded-control px-3 py-2 font-mono text-data text-ink placeholder:text-muted focus:outline-none transition-colors duration-120 hover:border-line-strong',
}

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { variant = 'modal', type = 'text', className = '', style, ...rest },
  ref,
) {
  // Paint the existing controlled range; do not change its value or events.
  const min = Number(rest.min ?? 0)
  const max = Number(rest.max ?? 100)
  const value = Number(rest.value ?? rest.defaultValue ?? (min + max) / 2)
  const progress =
    max > min
      ? Math.max(0, Math.min(100, ((value - min) / (max - min)) * 100))
      : 0
  const rangeStyle =
    type === 'range'
      ? ({ '--range-progress': `${progress}%` } as CSSProperties)
      : undefined
  return (
    <input
      ref={ref}
      type={type}
      style={{ ...rangeStyle, ...style }}
      className={`idle-control ${['range', 'checkbox', 'radio'].includes(type) ? '' : `${VARIANT_CLASSES[variant]} leading-[normal]`} ${type === 'number' ? 'font-mono tabular-nums' : ''} ${className}`}
      {...rest}
    />
  )
})
