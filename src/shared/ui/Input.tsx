import { forwardRef, type ComponentPropsWithoutRef } from 'react'

interface InputProps extends ComponentPropsWithoutRef<'input'> {
  variant?: 'modal' | 'config' | 'inline'
}

const VARIANT_CLASSES = {
  inline:
    'bg-field border border-line-strong text-ink placeholder:text-muted outline-none transition-colors duration-150 focus:border-accent',
  modal:
    'rounded-control border border-line-strong bg-field px-3 py-2.5 text-sm text-ink placeholder:text-muted outline-none transition-colors duration-150 focus:border-accent',
  config:
    'bg-field border border-line-strong rounded-control px-3 py-2.5 text-sm text-ink placeholder:text-muted focus:outline-none transition-colors duration-150 focus:border-accent',
}

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { variant = 'modal', type = 'text', className = '', ...rest },
  ref,
) {
  return (
    <input
      ref={ref}
      type={type}
      className={`idle-control ${['range', 'checkbox', 'radio'].includes(type) ? '' : VARIANT_CLASSES[variant]} ${type === 'number' ? 'font-mono tabular-nums' : ''} ${className}`}
      {...rest}
    />
  )
})
