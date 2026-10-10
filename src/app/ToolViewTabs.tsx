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
    <div className="shrink-0 flex gap-1 rounded-segmented border border-line-soft bg-panel p-[3px]">
      {TABS.map(({ view, label }) => {
        const active = toolView === view
        return (
          <button
            key={view}
            type="button"
            onClick={() => setToolView(view)}
            aria-pressed={active}
            className={`flex-1 flex items-center justify-center gap-1.5 rounded-segment px-7 py-1.5 text-data leading-[normal] font-sans font-medium transition-colors duration-120 ${
              active
                ? 'bg-line text-ink'
                : 'text-muted hover:bg-panel-raised hover:text-ink'
            }`}
          >
            {label}
          </button>
        )
      })}
    </div>
  )
}
