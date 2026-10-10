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
    <div className="shrink-0 flex gap-1 border-b border-line">
      {TABS.map(({ view, label, icon: Icon }) => {
        const active = toolView === view
        return (
          <button
            key={view}
            type="button"
            onClick={() => setToolView(view)}
            aria-pressed={active}
            className={`flex-1 flex items-center justify-center gap-1.5 border-b-2 px-2 py-1.5 text-detail font-medium transition-colors focus-visible:outline focus-visible:outline-accent ${
              active
                ? 'text-ink border-accent'
                : 'border-transparent text-muted hover:text-ink'
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
