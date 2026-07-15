import type { ComponentPropsWithoutRef, ReactNode } from 'react'

export type ButtonVariant =
  'primary' | 'secondary' | 'ghost' | 'danger' | 'success'

export type ButtonSize = 'xs' | 'sm' | 'md' | 'lg'

const BASE_CLASSES =
  'inline-flex items-center justify-center gap-1.5 rounded-lg font-medium select-none transition-[transform,background-color,border-color,color,box-shadow] duration-150 ease-out-quart focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/40 disabled:opacity-50 disabled:pointer-events-none motion-safe:hover:-translate-y-px motion-safe:active:translate-y-0 motion-safe:active:scale-[0.97]'

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary:
    'border border-amber-500/30 bg-amber-500/10 text-amber-100 hover:bg-amber-500/15 hover:border-amber-500/50 hover:shadow-glow-amber',
  secondary:
    'border border-zinc-700/60 bg-zinc-900/40 text-zinc-300 hover:border-amber-500/40 hover:text-amber-300 hover:bg-amber-500/5',
  ghost:
    'border border-transparent text-zinc-500 hover:text-zinc-300 hover:bg-white/[0.04]',
  danger:
    'border border-red-500/30 bg-red-500/10 text-red-300 hover:bg-red-500/15 hover:border-red-500/50 hover:shadow-glow-red',
  success:
    'border border-emerald-500/30 bg-emerald-500/10 text-emerald-300 hover:bg-emerald-500/15 hover:border-emerald-500/50 hover:shadow-glow-emerald',
}

const SIZE_CLASSES: Record<ButtonSize, string> = {
  xs: 'text-[10px] px-2 py-0.5',
  sm: 'text-[11px] px-3 py-1.5',
  md: 'text-sm px-4 py-2',
  lg: 'text-sm font-semibold py-2.5 px-4 rounded-xl',
}

export function buttonClasses(
  variant: ButtonVariant = 'secondary',
  size: ButtonSize = 'sm',
  block = false,
): string {
  return `${BASE_CLASSES} ${VARIANT_CLASSES[variant]} ${SIZE_CLASSES[size]} ${block ? 'w-full' : ''}`
}

interface ButtonProps extends ComponentPropsWithoutRef<'button'> {
  variant?: ButtonVariant
  size?: ButtonSize
  block?: boolean
}

export function Button({
  variant = 'secondary',
  size = 'sm',
  block = false,
  className = '',
  type = 'button',
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      className={`${buttonClasses(variant, size, block)} ${className}`}
      {...rest}
    />
  )
}

const ICON_SIZE_CLASSES: Record<ButtonSize, string> = {
  xs: 'w-5 h-5',
  sm: 'w-7 h-7',
  md: 'w-8 h-8',
  lg: 'w-10 h-10 rounded-xl',
}

interface IconButtonProps extends ComponentPropsWithoutRef<'button'> {
  label: string
  variant?: ButtonVariant
  size?: ButtonSize
  children: ReactNode
}

export function IconButton({
  label,
  variant = 'ghost',
  size = 'sm',
  className = '',
  type = 'button',
  children,
  ...rest
}: IconButtonProps) {
  return (
    <button
      type={type}
      aria-label={label}
      title={label}
      className={`${BASE_CLASSES} ${VARIANT_CLASSES[variant]} ${ICON_SIZE_CLASSES[size]} shrink-0 !px-0 !py-0 ${className}`}
      {...rest}
    >
      {children}
    </button>
  )
}
