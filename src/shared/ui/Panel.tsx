import type { ReactNode } from 'react'
import type { Tone } from './types'

export type PanelTone = Tone | 'idle' | 'success' | 'warning' | 'danger'
export type PanelSize = 'default' | 'compact' | 'hero'

export function resolveToolTone(
  available: boolean,
  engaged: boolean,
  hasError: boolean,
  activeTone: PanelTone = 'success',
): PanelTone {
  if (hasError) return 'danger'
  if (!available) return 'idle'
  if (engaged) return activeTone
  return 'neutral'
}

const TONE_CLASSES: Record<PanelTone, string> = {
  neutral: 'border-overlay-light/[0.06]',
  idle: 'border-overlay-light/[0.04] opacity-60',
  success: 'border-ok/30 shadow-glow-ok',
  warning: 'border-accent/30 shadow-glow-warn',
  danger: 'border-bad/30 shadow-glow-bad',
  ok: 'border-ok/30 shadow-glow-ok',
  warn: 'border-accent/30 shadow-glow-warn',
  bad: 'border-bad/30 shadow-glow-bad',
  info: 'border-info/70',
}

interface PanelProps {
  title: string
  action?: ReactNode
  leading?: ReactNode
  children: ReactNode
  className?: string
  compact?: boolean
  hero?: boolean
  tone?: PanelTone
  size?: PanelSize
  variant?: 'glass' | 'idle'
}

export function Panel({
  title,
  action,
  leading,
  children,
  className = '',
  compact = false,
  hero = false,
  tone = 'neutral',
  size = hero ? 'hero' : compact ? 'compact' : 'default',
  variant = 'glass',
}: PanelProps) {
  const headerPad =
    size === 'hero'
      ? 'px-4 py-3'
      : size === 'compact'
        ? 'px-3 py-1.5'
        : 'px-4 py-2.5'
  const bodyPad = size === 'compact' ? 'px-3 py-2' : 'px-4 py-3'
  const titleClass =
    size === 'hero'
      ? 'text-detail font-semibold text-ink-soft uppercase tracking-[0.16em] shrink-0'
      : 'text-caption font-semibold text-muted uppercase tracking-[0.14em] shrink-0'
  const effectiveTone = variant === 'idle' ? 'idle' : tone
  const shadow = ['ok', 'warn', 'bad', 'success', 'warning', 'danger'].includes(
    effectiveTone,
  )
    ? ''
    : 'shadow-panel'

  return (
    <section
      className={`rounded-panel border bg-panel-gradient from-panel-raised/30 to-panel/50 backdrop-blur-panel ${shadow} flex flex-col min-h-0 transition-[border-color,box-shadow,opacity,padding] duration-300 ${TONE_CLASSES[effectiveTone]} ${className}`}
    >
      <div
        className={`flex items-center justify-between gap-2 border-b border-overlay-light/[0.05] shrink-0 transition-[padding] duration-300 ${headerPad}`}
      >
        <div className="flex items-center gap-2 min-w-0">
          <h2 className={titleClass}>{title}</h2>
          {leading}
        </div>
        {action}
      </div>
      <div
        className={`flex-1 min-h-0 flex flex-col transition-[padding] duration-300 ${bodyPad}`}
      >
        {children}
      </div>
    </section>
  )
}
