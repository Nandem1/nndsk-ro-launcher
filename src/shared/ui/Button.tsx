import type { ComponentPropsWithoutRef, ReactNode } from 'react'
import type { Tone } from './types'

export type ButtonVariant =
  'primary' | 'secondary' | 'ghost' | 'danger' | 'success' | 'solid' | 'outline'

export type ButtonSize = 'xs' | 'sm' | 'md' | 'lg' | 'dialog' | 'dialog-sm'

const BASE_CLASSES =
  'idle-control inline-flex items-center justify-center gap-1.5 rounded-control font-medium select-none transition-colors duration-120 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent/50 disabled:pointer-events-none'

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary: 'button-primary bg-accent text-on-accent font-semibold',
  secondary: 'bg-panel-raised text-ink hover:bg-line',
  ghost: 'bg-transparent text-muted hover:text-ink hover:bg-panel-raised',
  danger: 'button-tonal-bad text-bad',
  success: 'button-tonal-ok text-ok',
  solid: 'font-semibold',
  outline: 'bg-panel-raised text-ink hover:bg-line',
}

const PRIMARY_TONE_CLASSES: Record<Tone, string> = {
  warn: 'button-tonal-warn text-warn',
  ok: VARIANT_CLASSES.success,
  bad: VARIANT_CLASSES.danger,
  info: 'button-tonal-info text-info',
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
  xs: 'text-xs px-3 py-1',
  sm: 'text-sm px-3.5 py-2',
  md: 'text-sm px-3.5 py-2',
  lg: 'text-sm font-semibold p-[13px] rounded-action',
  dialog: 'text-sm px-3.5 py-2',
  'dialog-sm': 'text-xs px-3.5 py-2',
}

export function buttonClasses(
  variant: ButtonVariant = 'secondary',
  size: ButtonSize = 'sm',
  block = false,
  tone?: Tone,
): string {
  const base = ['solid', 'outline'].includes(variant)
    ? `${BASE_CLASSES} flex-1`
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
  lg: 'w-10 h-10',
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
