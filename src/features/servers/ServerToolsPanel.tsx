import { invoke } from '@tauri-apps/api/core'
import { useCallback, useEffect, useState } from 'react'
import type { ServerConfig } from './servers.config'
import { useSettingsStore } from '../settings/settings.store'
import { Panel } from '../../shared/ui/Panel'

interface ToolInfo {
  found: boolean
  path: string | null
  label: string | null
}

interface DgVoodooStatus {
  cpl: ToolInfo
  d3dimmDll: ToolInfo
  ddrawDll: ToolInfo
  conf: ToolInfo
  configured: boolean
  needsInstall: boolean
  canAutoInstall: boolean
  canUninstall: boolean
  issues: string[]
}

interface ServerToolsStatus {
  gameDir: string
  openSetup: ToolInfo
  patcher: ToolInfo
  dgvoodoo: DgVoodooStatus
}

interface Props {
  server: ServerConfig | null
}

type ToolKind = 'opensetup' | 'patcher' | 'dgvoodoo'
type DotStatus = 'ok' | 'error' | 'neutral'

function StatusDot({ status }: { status: DotStatus }) {
  const classes: Record<DotStatus, string> = {
    ok: 'bg-emerald-500 shadow-[0_0_6px_rgba(16,185,129,0.5)]',
    error: 'bg-red-500 shadow-[0_0_6px_rgba(239,68,68,0.5)]',
    neutral: 'bg-zinc-600',
  }

  return (
    <span className={`inline-block w-2 h-2 rounded-full shrink-0 ${classes[status]}`} aria-hidden />
  )
}

function ToolRow({
  label,
  dotStatus,
  detail,
  warning,
  onAction,
  actionLabel,
  actionBusy,
  actionDisabled,
  onSecondary,
  secondaryLabel,
  secondaryBusy,
  secondaryDanger,
}: {
  label: string
  dotStatus: DotStatus
  detail?: string | null
  warning?: string | null
  onAction?: () => void
  actionLabel?: string
  actionBusy?: boolean
  actionDisabled?: boolean
  onSecondary?: () => void
  secondaryLabel?: string
  secondaryBusy?: boolean
  secondaryDanger?: boolean
}) {
  const actionClass =
    'text-xs px-2.5 py-1 rounded-md border border-zinc-700/80 text-zinc-300 hover:border-amber-500/50 hover:text-amber-400 hover:bg-amber-500/5 transition-colors shrink-0 disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:border-zinc-700 disabled:hover:text-zinc-300 disabled:hover:bg-transparent'

  const secondaryClass = secondaryDanger
    ? `${actionClass} hover:border-red-500/50 hover:text-red-400 hover:bg-red-500/5`
    : actionClass

  return (
    <div className="flex flex-col gap-1 py-2.5 border-b border-zinc-800/60 last:border-0">
      <div className="flex items-center gap-2.5 min-w-0">
        <StatusDot status={dotStatus} />
        <span className="text-sm text-zinc-200 shrink-0 w-20">{label}</span>
        {detail && (
          <span className="text-xs text-zinc-500 truncate flex-1 font-mono" title={detail}>
            {detail}
          </span>
        )}
        <div className="flex items-center gap-1.5 shrink-0">
          {onSecondary && secondaryLabel && (
            <button
              type="button"
              onClick={onSecondary}
              disabled={secondaryBusy}
              className={secondaryClass}
            >
              {secondaryBusy ? `${secondaryLabel}...` : secondaryLabel}
            </button>
          )}
          {onAction && actionLabel && (
            <button
              type="button"
              onClick={onAction}
              disabled={actionDisabled || actionBusy}
              className={actionClass}
            >
              {actionBusy ? `${actionLabel}...` : actionLabel}
            </button>
          )}
        </div>
      </div>
      {warning && (
        <p className="text-xs text-amber-400/90 pl-[18px] leading-relaxed">{warning}</p>
      )}
    </div>
  )
}

export function ServerToolsPanel({ server }: Props) {
  const selectedRunner = useSettingsStore((s) => s.selectedRunner)
  const [status, setStatus] = useState<ServerToolsStatus | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [opening, setOpening] = useState<ToolKind | null>(null)
  const [installingDgVoodoo, setInstallingDgVoodoo] = useState(false)
  const [uninstallingDgVoodoo, setUninstallingDgVoodoo] = useState(false)

  const refresh = useCallback(async () => {
    if (!server) {
      setStatus(null)
      setError(null)
      return
    }

    setLoading(true)
    setError(null)
    try {
      const result = await invoke<ServerToolsStatus>('scan_server_tools', { server })
      setStatus(result)
    } catch (err) {
      setStatus(null)
      setError(String(err))
    } finally {
      setLoading(false)
    }
  }, [server])

  useEffect(() => {
    refresh()
  }, [refresh])

  const handleInstallDgVoodoo = async () => {
    if (!server) return

    setInstallingDgVoodoo(true)
    setError(null)
    try {
      const result = await invoke<{ installed: string[]; status: ServerToolsStatus }>(
        'install_dgvoodoo',
        { server },
      )
      setStatus(result.status)
    } catch (err) {
      setError(String(err))
    } finally {
      setInstallingDgVoodoo(false)
    }
  }

  const handleUninstallDgVoodoo = async () => {
    if (!server) return

    const confirmed = window.confirm(
      '¿Desinstalar dgVoodoo de esta carpeta?\n\nSe eliminarán D3DImm.dll, DDraw.dll, dgVoodoo.conf y dgVoodooCpl.exe.',
    )
    if (!confirmed) return

    setUninstallingDgVoodoo(true)
    setError(null)
    try {
      const result = await invoke<{ removed: string[]; status: ServerToolsStatus }>(
        'uninstall_dgvoodoo',
        { server },
      )
      setStatus(result.status)
    } catch (err) {
      setError(String(err))
    } finally {
      setUninstallingDgVoodoo(false)
    }
  }

  const handleOpen = async (tool: ToolKind) => {
    if (!server) return

    setOpening(tool)
    setError(null)
    try {
      await invoke('launch_server_tool', {
        server: {
          ...server,
          runner: server.runner ?? selectedRunner ?? null,
        },
        tool,
        runner: server.runner ?? selectedRunner ?? null,
      })
    } catch (err) {
      setError(String(err))
    } finally {
      setOpening(null)
    }
  }

  if (!server) {
    return (
      <Panel title="Herramientas" className="shrink-0">
        <p className="text-sm text-zinc-600 text-center py-2">
          Selecciona un servidor para escanear herramientas
        </p>
      </Panel>
    )
  }

  const dg = status?.dgvoodoo
  const dgvoodooNeedsInstall = dg && !dg.configured && dg.canAutoInstall

  return (
    <Panel
      title="Herramientas"
      className="shrink-0"
      action={
        <button
          type="button"
          onClick={refresh}
          disabled={loading}
          className="text-xs text-zinc-600 hover:text-zinc-400 transition-colors disabled:opacity-40 px-1"
          title="Volver a escanear"
        >
          {loading ? '...' : '↻'}
        </button>
      }
    >
      {error && <p className="text-xs text-red-400 mb-2">{error}</p>}

      {!loading && status && (
        <div>
          <ToolRow
            label="OpenSetup"
            dotStatus={status.openSetup.found ? 'ok' : 'neutral'}
            detail={status.openSetup.label ?? (status.openSetup.found ? 'Detectado' : 'No encontrado')}
            onAction={status.openSetup.found ? () => handleOpen('opensetup') : undefined}
            actionLabel="Abrir"
            actionBusy={opening === 'opensetup'}
            actionDisabled={!status.openSetup.found}
          />
          <ToolRow
            label="Patcher"
            dotStatus={status.patcher.found ? 'ok' : 'neutral'}
            detail={status.patcher.label ?? (status.patcher.found ? 'Detectado' : 'No encontrado')}
            onAction={status.patcher.found ? () => handleOpen('patcher') : undefined}
            actionLabel="Abrir"
            actionBusy={opening === 'patcher'}
            actionDisabled={!status.patcher.found}
          />
          <ToolRow
            label="dgVoodoo"
            dotStatus={status.dgvoodoo.configured ? 'ok' : 'error'}
            detail={
              status.dgvoodoo.configured
                ? 'D3DImm · DDraw · conf OK'
                : 'No detectado'
            }
            warning={
              !status.dgvoodoo.configured && status.dgvoodoo.issues.length > 0
                ? status.dgvoodoo.issues.join(' · ')
                : undefined
            }
            onAction={
              dgvoodooNeedsInstall
                ? handleInstallDgVoodoo
                : status.dgvoodoo.cpl.found
                  ? () => handleOpen('dgvoodoo')
                  : undefined
            }
            actionLabel={dgvoodooNeedsInstall ? 'Instalar' : 'Configurar'}
            actionBusy={dgvoodooNeedsInstall ? installingDgVoodoo : opening === 'dgvoodoo'}
            onSecondary={
              status.dgvoodoo.canUninstall ? handleUninstallDgVoodoo : undefined
            }
            secondaryLabel="Desinstalar"
            secondaryBusy={uninstallingDgVoodoo}
            secondaryDanger
          />
        </div>
      )}

      {loading && !status && (
        <p className="text-xs text-zinc-600 py-2 text-center">Escaneando carpeta...</p>
      )}
    </Panel>
  )
}
