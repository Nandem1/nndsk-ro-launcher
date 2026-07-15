import type { ReactNode } from 'react'

export type PanelTone = 'neutral' | 'idle' | 'success' | 'warning' | 'danger'

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
  neutral: 'border-white/[0.06]',
  idle: 'border-white/[0.04] opacity-60',
  success: 'border-emerald-500/30 shadow-glow-emerald',
  warning: 'border-amber-500/30 shadow-glow-amber',
  danger: 'border-red-500/30 shadow-glow-red',
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
}: PanelProps) {
  const headerPad = hero ? 'px-4 py-3' : compact ? 'px-3 py-1.5' : 'px-4 py-2.5'
  const bodyPad = hero ? 'px-4 py-3' : compact ? 'px-3 py-2' : 'px-4 py-3'
  const titleClass = hero
    ? 'text-[11px] font-semibold text-zinc-400 uppercase tracking-[0.16em] shrink-0'
    : 'text-[10px] font-semibold text-zinc-500 uppercase tracking-[0.14em] shrink-0'

  return (
    <section
      className={`rounded-xl border bg-gradient-to-b from-zinc-800/30 to-zinc-900/50 backdrop-blur-sm shadow-glass flex flex-col min-h-0 transition-[border-color,box-shadow,opacity,padding] duration-300 ${TONE_CLASSES[tone]} ${className}`}
    >
      <div
        className={`flex items-center justify-between gap-2 border-b border-white/[0.05] shrink-0 transition-[padding] duration-300 ${headerPad}`}
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
