import { forwardRef, type ComponentPropsWithoutRef } from 'react'

interface InputProps extends ComponentPropsWithoutRef<'input'> {
  variant?: 'modal' | 'config'
}

const VARIANT_CLASSES = {
  modal:
    'rounded-control border border-line/80 bg-surface/70 px-3 py-2.5 text-sm text-ink outline-none focus:border-accent/60',
  config:
    'bg-surface/60 border border-line/80 rounded-control px-3 py-2.5 text-sm text-ink placeholder:text-line-strong focus:outline-none focus:border-accent/60 focus:ring-1 focus:ring-accent/20',
}

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { variant = 'modal', className = '', ...rest },
  ref,
) {
  return (
    <input
      ref={ref}
      className={`${VARIANT_CLASSES[variant]} ${className}`}
      {...rest}
    />
  )
})
