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
  neutral: 'border-t-line-strong',
  idle: 'border-t-line-strong opacity-60',
  success: 'border-t-ok',
  warning: 'border-t-warn',
  danger: 'border-t-bad',
  ok: 'border-t-ok',
  warn: 'border-t-warn',
  bad: 'border-t-bad',
  info: 'border-t-info',
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
    'text-panel-title font-panel-title text-ink tracking-panel-title shrink-0'
  const effectiveTone = variant === 'idle' ? 'idle' : tone

  return (
    <section
      className={`rounded-panel border-t-2 bg-panel flex flex-col min-h-0 transition-colors duration-150 ${TONE_CLASSES[effectiveTone]} ${className}`}
    >
      <div
        className={`flex items-center justify-between gap-2 border-b border-line shrink-0 ${headerPad}`}
      >
        <div className="flex items-center gap-2 min-w-0">
          <h2 className={titleClass}>{title}</h2>
          {leading}
        </div>
        {action}
      </div>
      <div className={`flex-1 min-h-0 flex flex-col ${bodyPad}`}>
        {children}
      </div>
    </section>
  )
}
