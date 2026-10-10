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
  neutral: '',
  idle: 'panel-idle',
  success: '',
  warning: '',
  danger: '',
  ok: '',
  warn: '',
  bad: '',
  info: '',
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
  const headerPad = 'px-[18px] pt-4 pb-2'
  const bodyPad = 'px-[18px] pt-2 pb-4'
  const titleClass =
    'text-panel-title font-panel-title font-sans tracking-panel-title shrink-0'
  const effectiveTone = variant === 'idle' ? 'idle' : tone

  return (
    <section
      className={`rounded-panel border border-line-soft bg-panel flex flex-col min-h-0 transition-colors duration-120 ${TONE_CLASSES[effectiveTone]} ${size === 'hero' ? 'panel-hero' : ''} ${className}`}
    >
      <div
        className={`flex items-center justify-between gap-2 shrink-0 ${headerPad}`}
      >
        <div className="flex items-center gap-2 min-w-0">
          <h2
            className={`${titleClass} ${effectiveTone === 'idle' ? 'text-muted' : 'text-ink'}`}
          >
            {title}
          </h2>
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
