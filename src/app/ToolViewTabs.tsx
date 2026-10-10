import { useUiModeStore, type ToolView } from './uiMode.store'
import { segmentedClasses, segmentClasses } from '../shared/ui/segmentedControl'

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
    <div className={segmentedClasses()}>
      {TABS.map(({ view, label }) => {
        const active = toolView === view
        return (
          <button
            key={view}
            type="button"
            onClick={() => setToolView(view)}
            aria-pressed={active}
            className={segmentClasses(active)}
          >
            {label}
          </button>
        )
      })}
    </div>
  )
}
