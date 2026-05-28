import type { ReactNode } from 'react'

export type PanelTone =
  | 'neutral'
  | 'idle'
  | 'success'
  | 'warning'
  | 'danger'

const TONE_CLASSES: Record<PanelTone, string> = {
  neutral: 'border-zinc-800/80',
  idle: 'border-zinc-800/60 opacity-60',
  success: 'border-emerald-500/40',
  warning: 'border-amber-500/40',
  danger: 'border-red-500/40',
}

interface PanelProps {
  title: string
  action?: ReactNode
  leading?: ReactNode
  children: ReactNode
  className?: string
  compact?: boolean
  tone?: PanelTone
}

export function Panel({
  title,
  action,
  leading,
  children,
  className = '',
  compact = false,
  tone = 'neutral',
}: PanelProps) {
  return (
    <section
      className={`rounded-xl border bg-zinc-900/40 backdrop-blur-sm flex flex-col min-h-0 transition-[border-color,box-shadow] duration-300 ${TONE_CLASSES[tone]} ${className}`}
    >
      <div
        className={`flex items-center justify-between gap-2 border-b border-zinc-800/80 shrink-0 ${
          compact ? 'px-3 py-1.5' : 'px-4 py-2.5'
        }`}
      >
        <div className="flex items-center gap-2 min-w-0">
          <h2 className="text-[10px] font-semibold text-zinc-500 uppercase tracking-[0.14em] shrink-0">
            {title}
          </h2>
          {leading}
        </div>
        {action}
      </div>
      <div className={`flex-1 min-h-0 flex flex-col ${compact ? 'px-3 py-2' : 'px-4 py-3'}`}>
        {children}
      </div>
    </section>
  )
}
