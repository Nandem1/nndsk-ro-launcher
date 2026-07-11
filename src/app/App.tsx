import { AppHeader } from './AppHeader'
import { useAppInit } from './useAppInit'
import { LoadingScreen } from './LoadingScreen'
import { ServerList } from '../features/servers/ServerList'
import { ServerToolsPanel } from '../features/servers/ServerToolsPanel'
import { LaunchButton } from '../features/launcher/LaunchButton'
import { AutopotPanel } from '../features/autopot/AutopotPanel'
import { SpammerPanel } from '../features/spammer/SpammerPanel'
import { AutobuffPanel } from '../features/autobuff/AutobuffPanel'
import { UnifiedLogPanel } from '../features/logs/LogPanels'
import { RunnerSelector } from '../features/settings/RunnerSelector'
import { AdvancedSettings } from '../features/settings/AdvancedSettings'
import { PrefixResetButton } from '../features/settings/PrefixResetButton'
import { useLauncherEvents } from '../features/launcher/useLauncherEvents'

export function App() {
  const { ready } = useAppInit()
  useLauncherEvents()

  if (!ready) return <LoadingScreen />

  return (
    <div className="h-full flex flex-col">
      <AppHeader />

      <main className="flex-1 min-h-0 grid grid-cols-[minmax(0,300px)_1fr] gap-3 p-3">
        <div className="flex flex-col min-h-0">
          <div className="flex-1 min-h-0 overflow-y-auto flex flex-col gap-2.5 pr-0.5">
            <ServerList />
            <RunnerSelector />
            <AdvancedSettings />
          </div>
          <div className="shrink-0 pb-2.5">
            <PrefixResetButton />
          </div>
          <LaunchButton />
        </div>

        <div className="flex flex-col gap-2.5 min-h-0">
          <ServerToolsPanel />

          <div className="grid grid-cols-3 gap-2.5 shrink-0 items-stretch">
            <AutopotPanel />
            <AutobuffPanel />
            <SpammerPanel />
          </div>

          <UnifiedLogPanel />
        </div>
      </main>
    </div>
  )
}
