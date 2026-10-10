import type { ComponentPropsWithoutRef, ReactNode } from 'react'
import type { Tone } from './types'

export type ButtonVariant =
  'primary' | 'secondary' | 'ghost' | 'danger' | 'success' | 'solid' | 'outline'

export type ButtonSize = 'xs' | 'sm' | 'md' | 'lg' | 'dialog' | 'dialog-sm'

const BASE_CLASSES =
  'inline-flex items-center justify-center gap-1.5 rounded-control font-medium select-none transition-[transform,background-color,border-color,color,box-shadow] duration-150 ease-out-quart focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/40 disabled:opacity-50 disabled:pointer-events-none motion-safe:hover:-translate-y-px motion-safe:active:translate-y-0 motion-safe:active:scale-[0.97]'

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary:
    'border border-accent/30 bg-accent/10 text-accent-ink hover:bg-accent/15 hover:border-accent/50 hover:shadow-glow-warn',
  secondary:
    'border border-line/60 bg-panel/40 text-ink-dim hover:border-accent/40 hover:text-accent-light hover:bg-accent/5',
  ghost:
    'border border-transparent text-muted hover:text-ink-dim hover:bg-overlay-light/[0.04]',
  danger:
    'border border-bad/30 bg-bad/10 text-bad-soft hover:bg-bad/15 hover:border-bad/50 hover:shadow-glow-bad',
  success:
    'border border-ok/30 bg-ok/10 text-ok-soft hover:bg-ok/15 hover:border-ok/50 hover:shadow-glow-ok',
  solid: 'font-semibold text-surface disabled:opacity-40',
  outline: 'border border-line text-ink-soft hover:text-ink',
}

const PRIMARY_TONE_CLASSES: Record<Tone, string> = {
  warn: VARIANT_CLASSES.primary,
  ok: VARIANT_CLASSES.success,
  bad: VARIANT_CLASSES.danger,
  info: 'border border-info/70 bg-info/15 text-info-soft',
  neutral: VARIANT_CLASSES.secondary,
}

const SOLID_TONE_CLASSES: Record<Tone, string> = {
  warn: 'bg-accent',
  ok: 'bg-ok',
  bad: 'bg-bad/10',
  info: 'bg-info/15',
  neutral: 'bg-panel-raised',
}

function variantClasses(variant: ButtonVariant, tone?: Tone): string {
  if (variant === 'solid')
    return `${VARIANT_CLASSES.solid} ${SOLID_TONE_CLASSES[tone ?? 'warn']}`
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
    ? 'flex-1 rounded-panel'
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
