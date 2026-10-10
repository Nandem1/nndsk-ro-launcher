import { useUiModeStore, type ToolView } from './uiMode.store'

const TABS: {
  view: ToolView
  label: string
}[] = [
  { view: 'combat', label: 'Combate' },
  { view: 'buffs', label: 'Buffs' },
]

export function ToolViewTabs() {
  const toolView = useUiModeStore((s) => s.toolView)
  const setToolView = useUiModeStore((s) => s.setToolView)

  return (
    <div className="shrink-0 flex gap-1">
      {TABS.map(({ view, label }) => {
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
            {label}
          </button>
        )
      })}
    </div>
  )
}
