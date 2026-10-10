import { getVersion } from '@tauri-apps/api/app'
import { useEffect, useState } from 'react'
import { useLauncherStore } from '../features/launcher/launcher.store'
import { useUpdaterStore } from '../features/updater/updater.store'
import { updateCopy } from '../features/updater/updater.logic'
import { StatusDot } from '../shared/ui/StatusDot'
import { useUiModeStore } from './uiMode.store'
import { WindowControls } from './WindowControls'
import { DataText } from '../shared/ui/DataText'

function IngameStatusChip() {
  const launching = useLauncherStore((s) => s.status === 'launching')
  const clients = useLauncherStore((s) => s.clients)
  const running = clients.filter((client) => client.status === 'running').length

  return (
    <div className="flex items-center gap-2 px-3 py-1.5 rounded-pill border border-line bg-panel ">
      <StatusDot status={launching ? 'warning' : 'ok'} pulse />
      <span className="text-detail text-ink font-medium truncate max-w-[220px]">
        {launching ? 'Iniciando...' : 'En juego'}
        <DataText>
          {clients.length === 1
            ? ` · ${clients[0].serverName}`
            : ` · ${running}/${clients.length} clientes`}
        </DataText>
      </span>
    </div>
  )
}

function VersionChip() {
  const snapshot = useUpdaterStore((s) => s.snapshot)
  const [fallbackVersion, setFallbackVersion] = useState<string | null>(null)
  const copy = updateCopy(snapshot)

  useEffect(() => {
    let cancelled = false
    void getVersion()
      .then((version) => {
        if (!cancelled) setFallbackVersion(version)
      })
      .catch(() => {
        if (!cancelled) setFallbackVersion(null)
      })
    return () => {
      cancelled = true
    }
  }, [])

  const version = snapshot?.currentVersion ?? fallbackVersion
  if (!version) return null

  return (
    <div className="flex items-center gap-2 px-3 py-1.5 rounded-pill border border-line bg-panel">
      <StatusDot status={copy.dot} pulse={copy.dot === 'warning'} />
      <span className="text-detail text-muted font-mono font-medium">
        v{version}
      </span>
    </div>
  )
}

export function AppHeader() {
  const ingame = useUiModeStore((s) => s.mode === 'ingame')

  return (
    <header className="shrink-0 flex items-end justify-between px-4 py-2.5 border-b border-line bg-surface ">
      <div className="min-w-0 flex-1" data-tauri-drag-region>
        <h1 className="text-xl font-wordmark font-bold tracking-tight">
          <span className="text-accent">RO</span>
          <span className="text-ink">-Launcher</span>
        </h1>
        <p className="text-xs text-muted mt-0.5">Ragnarok Online · Linux</p>
      </div>
      <div className="flex items-center gap-2" data-tauri-drag-region="false">
        <VersionChip />
        {ingame ? (
          <IngameStatusChip />
        ) : (
          <p className="text-detail text-muted tracking-wide">
            Developed by: <span className="text-muted font-medium">nndsk</span>
          </p>
        )}
        <WindowControls />
      </div>
    </header>
  )
}
