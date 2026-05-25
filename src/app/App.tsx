import { AppHeader } from './AppHeader'
import { useAppInit } from './useAppInit'
import { ServerList } from '../features/servers/ServerList'
import { ServerToolsPanel } from '../features/servers/ServerToolsPanel'
import { LaunchButton } from '../features/launcher/LaunchButton'
import { AutopotPanel } from '../features/autopot/AutopotPanel'
import { UnifiedLogPanel } from '../features/logs/LogPanels'
import { RunnerSelector } from '../features/settings/RunnerSelector'
import { SystemStatusBanner } from '../features/settings/SystemStatusBanner'
import { PrefixResetButton } from '../features/settings/PrefixResetButton'
import { useLauncherEvents } from '../features/launcher/useLauncherEvents'

export function App() {
  useAppInit()
  useLauncherEvents()

  return (
    <div className="h-full flex flex-col">
      <AppHeader />

      <main className="flex-1 min-h-0 grid grid-cols-[minmax(0,340px)_1fr] gap-4 p-4">
        <div className="flex flex-col min-h-0">
          <div className="flex-1 min-h-0 overflow-y-auto flex flex-col gap-3 pr-0.5">
            <ServerList />
            <RunnerSelector />
            <SystemStatusBanner />
            <PrefixResetButton />
          </div>
          <div className="shrink-0 pt-3">
            <LaunchButton />
          </div>
        </div>

        <div className="flex flex-col gap-3 min-h-0">
          <ServerToolsPanel />
          <AutopotPanel />
          <UnifiedLogPanel />
        </div>
      </main>
    </div>
  )
}
