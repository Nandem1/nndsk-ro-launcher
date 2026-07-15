import { Swords, Sparkles } from 'lucide-react'
import type { ComponentType } from 'react'
import { useUiModeStore, type ToolView } from './uiMode.store'

const TABS: {
  view: ToolView
  label: string
  icon: ComponentType<{ className?: string }>
}[] = [
  { view: 'combat', label: 'Combate', icon: Swords },
  { view: 'buffs', label: 'Buffs', icon: Sparkles },
]

export function ToolViewTabs() {
  const toolView = useUiModeStore((s) => s.toolView)
  const setToolView = useUiModeStore((s) => s.setToolView)

  return (
    <div className="shrink-0 flex gap-1 rounded-lg border border-zinc-800/60 bg-zinc-950/40 p-1">
      {TABS.map(({ view, label, icon: Icon }) => {
        const active = toolView === view
        return (
          <button
            key={view}
            type="button"
            onClick={() => setToolView(view)}
            aria-pressed={active}
            className={`flex-1 flex items-center justify-center gap-1.5 rounded-md px-2 py-1.5 text-[11px] font-medium transition-colors motion-safe:active:scale-[0.98] ${
              active
                ? 'bg-amber-500/15 text-amber-200 border border-amber-500/40'
                : 'border border-transparent text-zinc-500 hover:text-zinc-300 hover:bg-white/[0.04]'
            }`}
          >
            <Icon className="w-3.5 h-3.5 shrink-0" />
            {label}
          </button>
        )
      })}
    </div>
  )
}
