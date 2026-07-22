import { useRef } from 'react'
import { ChevronsLeft } from 'lucide-react'
import { AppHeader } from './AppHeader'
import { useAppInit } from './useAppInit'
import { LoadingScreen } from './LoadingScreen'
import { ServerList } from '../features/servers/ServerList'
import { ServerToolsPanel } from '../features/servers/ServerToolsPanel'
import { LaunchButton } from '../features/launcher/LaunchButton'
import { IngameRail } from '../features/launcher/IngameRail'
import { ActiveClients } from '../features/launcher/ActiveClients'
import { AutopotPanel } from '../features/autopot/AutopotPanel'
import { SpammerPanel } from '../features/spammer/SpammerPanel'
import { AutobuffPanel } from '../features/autobuff/AutobuffPanel'
import { UnifiedLogPanel } from '../features/logs/LogPanels'
import { AdvancedSettings } from '../features/settings/AdvancedSettings'
import { PrefixResetButton } from '../features/settings/PrefixResetButton'
import { useLauncherEvents } from '../features/launcher/useLauncherEvents'
import { IconButton } from '../shared/ui/Button'
import { ToolViewTabs } from './ToolViewTabs'
import { useUiModeStore } from './uiMode.store'
import { useUiModeTransition } from './useUiModeTransition'
import { useTransformLayoutTransition } from './useTransformLayoutTransition'
import { StartupNotice } from './StartupNotice'
import { MaintenanceNotice } from './MaintenanceNotice'
import { useSelectedRuntimeStatus } from '../features/settings/useSelectedRuntimeStatus'

const RAIL_EXPANDED_PX = 300
const RAIL_COLLAPSED_PX = 64

export function App() {
  const { phase, errors, notices, retrying, retry, dismissNotices } =
    useAppInit()
  useLauncherEvents()
  useUiModeTransition()
  useSelectedRuntimeStatus()

  const mode = useUiModeStore((s) => s.mode)
  const railPeek = useUiModeStore((s) => s.railPeek)
  const toggleRailPeek = useUiModeStore((s) => s.toggleRailPeek)
  const toolView = useUiModeStore((s) => s.toolView)
  const railExpanded = mode === 'prep' || railPeek
  const railWidth = railExpanded ? RAIL_EXPANDED_PX : RAIL_COLLAPSED_PX
  const contentRef = useRef<HTMLDivElement>(null)
  useTransformLayoutTransition(
    railExpanded,
    contentRef,
    RAIL_EXPANDED_PX - RAIL_COLLAPSED_PX,
  )

  if (phase === 'loading') return <LoadingScreen />

  return (
    <div className="h-full flex flex-col">
      <AppHeader />
      {phase === 'degraded' && (
        <StartupNotice
          errors={errors}
          retrying={retrying}
          onRetry={() => void retry()}
        />
      )}
      <MaintenanceNotice notices={notices} onDismiss={dismissNotices} />

      <main
        style={{ gridTemplateColumns: `${railWidth}px 1fr` }}
        className="flex-1 min-h-0 grid gap-3 p-3"
      >
        <div className="flex flex-col min-h-0 min-w-0 overflow-hidden">
          {railExpanded ? (
            <div
              key="rail-full"
              className="flex flex-col min-h-0 flex-1 animate-rail-expand"
            >
              {mode === 'ingame' && (
                <div className="shrink-0 flex justify-end pb-2">
                  <IconButton
                    label="Minimizar panel"
                    variant="ghost"
                    size="sm"
                    onClick={toggleRailPeek}
                  >
                    <ChevronsLeft className="w-4 h-4" />
                  </IconButton>
                </div>
              )}
              <div className="flex-1 min-h-0 overflow-y-auto flex flex-col gap-2.5 pr-0.5">
                <ServerList />
                <ActiveClients />
                <AdvancedSettings />
              </div>
              <div className="shrink-0 pb-2.5">
                <PrefixResetButton />
              </div>
              <LaunchButton />
            </div>
          ) : (
            <IngameRail key="rail-slim" />
          )}
        </div>

        <div
          ref={contentRef}
          className="flex flex-col gap-2.5 min-h-0 will-change-transform"
        >
          {mode === 'prep' && <ServerToolsPanel />}

          <ToolViewTabs />

          <div className="flex-1 min-h-0">
            {toolView === 'combat' ? (
              <div className="grid h-full grid-cols-2 gap-2.5 items-stretch stagger-children">
                <AutopotPanel />
                <SpammerPanel />
              </div>
            ) : (
              <div className="h-full stagger-children">
                <AutobuffPanel />
              </div>
            )}
          </div>

          <div className="shrink-0 flex h-44">
            <UnifiedLogPanel />
          </div>
        </div>
      </main>
    </div>
  )
}
