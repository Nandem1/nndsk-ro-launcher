import { forwardRef, type ComponentPropsWithoutRef } from 'react'

interface InputProps extends ComponentPropsWithoutRef<'input'> {
  variant?: 'modal' | 'config'
}

const VARIANT_CLASSES = {
  modal:
    'rounded-control border border-line-strong bg-field px-3 py-2.5 text-sm text-ink placeholder:text-muted outline-none transition-colors duration-150 focus:border-accent',
  config:
    'bg-field border border-line-strong rounded-control px-3 py-2.5 text-sm text-ink placeholder:text-muted focus:outline-none transition-colors duration-150 focus:border-accent',
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
