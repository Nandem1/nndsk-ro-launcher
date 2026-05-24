import { useEffect } from 'react'
import { AppHeader } from './AppHeader'
import { ServerList } from '../features/servers/ServerList'
import { ServerToolsPanel } from '../features/servers/ServerToolsPanel'
import { LaunchButton } from '../features/launcher/LaunchButton'
import { LogPanel } from '../features/logs/LogPanel'
import { RunnerSelector } from '../features/settings/RunnerSelector'
import { AudioStatusBanner } from '../features/settings/AudioStatusBanner'
import { PrefixResetButton } from '../features/settings/PrefixResetButton'
import { useServersStore } from '../features/servers/servers.store'

export function App() {
  const { getSelected, loadServers } = useServersStore()

  useEffect(() => {
    loadServers()
  }, [])

  const selected = getSelected()

  return (
    <div className="h-full flex flex-col">
      <AppHeader />

      <main className="flex-1 min-h-0 grid grid-cols-[minmax(0,340px)_1fr] gap-4 p-4">
        {/* Columna izquierda: servidor + lanzamiento */}
        <div className="flex flex-col gap-4 min-h-0">
          <ServerList />
          <RunnerSelector />
          <AudioStatusBanner />
          <PrefixResetButton />
          <div className="mt-auto">
            <LaunchButton server={selected} />
          </div>
        </div>

        {/* Columna derecha: herramientas + logs */}
        <div className="flex flex-col gap-4 min-h-0">
          <ServerToolsPanel server={selected} />
          <LogPanel />
        </div>
      </main>
    </div>
  )
}
