import type { ComponentPropsWithoutRef, ReactNode } from 'react'
import type { Tone } from './types'

export type ButtonVariant =
  'primary' | 'secondary' | 'ghost' | 'danger' | 'success' | 'solid' | 'outline'

export type ButtonSize = 'xs' | 'sm' | 'md' | 'lg' | 'dialog' | 'dialog-sm'

const BASE_CLASSES =
  'inline-flex items-center justify-center gap-1.5 rounded-control font-medium select-none transition-colors duration-150 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent disabled:opacity-50 disabled:pointer-events-none'

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary:
    'border border-accent bg-accent text-on-accent font-semibold hover:border-ink',
  secondary:
    'border border-line-strong bg-transparent text-ink hover:border-accent hover:bg-panel-raised',
  ghost:
    'border border-transparent text-muted hover:text-ink hover:bg-panel-raised',
  danger:
    'border border-bad/50 bg-transparent text-bad hover:bg-panel-raised hover:border-bad',
  success:
    'border border-ok/50 bg-transparent text-ok hover:bg-panel-raised hover:border-ok',
  solid: 'font-semibold disabled:opacity-40',
  outline:
    'border border-line-strong bg-transparent text-ink hover:border-accent hover:bg-panel-raised',
}

const PRIMARY_TONE_CLASSES: Record<Tone, string> = {
  warn: 'border border-warn/50 bg-transparent text-warn hover:border-warn hover:bg-panel-raised',
  ok: VARIANT_CLASSES.success,
  bad: VARIANT_CLASSES.danger,
  info: 'border border-info/50 bg-transparent text-info hover:border-info hover:bg-panel-raised',
  neutral: VARIANT_CLASSES.secondary,
}

const SOLID_TONE_CLASSES: Record<Tone, string> = {
  warn: 'bg-warn text-on-accent',
  ok: 'bg-ok text-on-accent',
  bad: 'bg-bad text-on-accent',
  info: 'bg-info text-on-accent',
  neutral: 'bg-panel-raised text-ink',
}

function variantClasses(variant: ButtonVariant, tone?: Tone): string {
  if (variant === 'solid')
    return `${VARIANT_CLASSES.solid} ${tone ? SOLID_TONE_CLASSES[tone] : 'bg-accent text-on-accent'}`
  if (tone && ['primary', 'danger', 'success'].includes(variant))
    return PRIMARY_TONE_CLASSES[tone]
  return VARIANT_CLASSES[variant]
}

const SIZE_CLASSES: Record<ButtonSize, string> = {
  xs: 'text-caption px-2 py-0.5',
  sm: 'text-detail px-3 py-1.5',
  md: 'text-sm px-4 py-2',
  lg: 'text-sm font-semibold py-2.5 px-4 rounded-panel',
  dialog: 'text-sm py-2.5',
  'dialog-sm': 'text-xs py-2.5',
}

export function buttonClasses(
  variant: ButtonVariant = 'secondary',
  size: ButtonSize = 'sm',
  block = false,
  tone?: Tone,
): string {
  const base = ['solid', 'outline'].includes(variant)
    ? 'flex-1 rounded-panel transition-colors duration-150 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent'
    : BASE_CLASSES
  return `${base} ${variantClasses(variant, tone)} ${SIZE_CLASSES[size]} ${block ? 'w-full' : ''}`
}

interface ButtonProps extends ComponentPropsWithoutRef<'button'> {
  variant?: ButtonVariant
  size?: ButtonSize
  block?: boolean
  tone?: Tone
}

export function Button({
  variant = 'secondary',
  size = 'sm',
  block = false,
  tone,
  className = '',
  type = 'button',
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      className={`${buttonClasses(variant, size, block, tone)} ${className}`}
      {...rest}
    />
  )
}

type IconButtonSize = Exclude<ButtonSize, 'dialog' | 'dialog-sm'>

const ICON_SIZE_CLASSES: Record<IconButtonSize, string> = {
  xs: 'w-5 h-5',
  sm: 'w-7 h-7',
  md: 'w-8 h-8',
  lg: 'w-10 h-10 rounded-panel',
}

interface IconButtonProps extends ComponentPropsWithoutRef<'button'> {
  label: string
  variant?: Exclude<ButtonVariant, 'solid' | 'outline'>
  size?: IconButtonSize
  tone?: Tone
  children: ReactNode
}

export function IconButton({
  label,
  variant = 'ghost',
  size = 'sm',
  tone,
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
      className={`${BASE_CLASSES} ${variantClasses(variant, tone)} ${ICON_SIZE_CLASSES[size]} shrink-0 !px-0 !py-0 ${className}`}
      {...rest}
    >
      {children}
    </button>
  )
}
