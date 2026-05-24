import type { ReactNode } from 'react'

interface PanelProps {
  title: string
  action?: ReactNode
  children: ReactNode
  className?: string
}

export function Panel({ title, action, children, className = '' }: PanelProps) {
  return (
    <section
      className={`rounded-xl border border-zinc-800/80 bg-zinc-900/40 backdrop-blur-sm flex flex-col min-h-0 ${className}`}
    >
      <div className="flex items-center justify-between px-4 py-2.5 border-b border-zinc-800/80 shrink-0">
        <h2 className="text-[11px] font-semibold text-zinc-500 uppercase tracking-[0.14em]">
          {title}
        </h2>
        {action}
      </div>
      <div className="px-4 py-3 flex-1 min-h-0 flex flex-col">{children}</div>
    </section>
  )
}
