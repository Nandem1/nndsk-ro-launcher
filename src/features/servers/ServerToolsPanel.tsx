import { RotateCw } from 'lucide-react'
import type { ServerToolsStatus, ToolInfo, ToolKind } from '../../shared/types'
import { Panel } from '../../shared/ui/Panel'
import { StatusDot } from '../../shared/ui/StatusDot'
import { buttonClasses } from '../../shared/ui/Button'
import { useSelectedServer } from './useSelectedServer'
import { useServerTools } from './useServerTools'
import { useCurrentAdvancedStatus } from '../settings/useSelectedRuntimeStatus'

export function ServerToolsPanel() {
  const server = useSelectedServer()
  const prefixConfigured = useCurrentAdvancedStatus()?.readyToLaunch ?? false
  const {
    status,
    loading,
    error,
    opening,
    installingDgVoodoo,
    uninstallingDgVoodoo,
    busy,
    refresh,
    handleInstallDgVoodoo,
    handleUninstallDgVoodoo,
    handleOpen,
  } = useServerTools(server)

  if (!server) {
    return (
      <Panel
        title="Herramientas"
        compact
        className="shrink-0 animate-fade-rise"
      >
        <p className="text-[11px] text-zinc-600 text-center py-1">
          Selecciona un servidor
        </p>
      </Panel>
    )
  }

  const dg = status?.dgvoodoo
  const dgvoodooNeedsInstall = dg && dg.needsInstall && dg.canAutoInstall

  return (
    <Panel
      title="Herramientas"
      compact
      className="shrink-0 animate-fade-rise"
      action={
        <button
          type="button"
          onClick={refresh}
          disabled={busy}
          className="text-zinc-600 hover:text-zinc-400 transition-colors disabled:opacity-40"
          title="Volver a escanear"
        >
          <RotateCw className={`w-3 h-3 ${loading ? 'animate-spin' : ''}`} />
        </button>
      }
    >
      {error && <p className="text-[10px] text-red-400 mb-1.5">{error}</p>}

      {status && (
        <>
          <ToolsGrid
            status={status}
            prefixConfigured={prefixConfigured}
            dgvoodooNeedsInstall={!!dgvoodooNeedsInstall}
            opening={opening}
            installingDgVoodoo={installingDgVoodoo}
            uninstallingDgVoodoo={uninstallingDgVoodoo}
            busy={busy}
            onOpen={handleOpen}
            onInstallDgVoodoo={handleInstallDgVoodoo}
            onUninstallDgVoodoo={handleUninstallDgVoodoo}
          />
          <ClientDiagnostics status={status} />
          {!!status.dgvoodoo.issues.length && (
            <div className="mt-2 rounded-lg border border-amber-500/10 bg-amber-500/5 px-2.5 py-2">
              {status.dgvoodoo.issues.map((issue) => (
                <p
                  key={issue}
                  className="text-[10px] leading-snug text-amber-400/80"
                >
                  {issue}
                </p>
              ))}
            </div>
          )}
        </>
      )}

      {loading && !status && (
        <p className="text-[10px] text-zinc-600 py-1 text-center">
          Escaneando...
        </p>
      )}
    </Panel>
  )
}

function ClientDiagnostics({ status }: { status: ServerToolsStatus }) {
  const diagnostics = status.diagnostics
  if (
    !diagnostics.architecture &&
    diagnostics.graphicsApis.length === 0 &&
    diagnostics.warnings.length === 0
  ) {
    return null
  }

  return (
    <div className="mt-2 rounded-lg border border-white/[0.04] bg-zinc-950/30 px-2.5 py-2">
      <p className="text-[10px] text-zinc-500">
        Cliente {diagnostics.architecture ?? 'PE'}
        {diagnostics.graphicsApis.length
          ? ` · ${diagnostics.graphicsApis.join(' + ')}`
          : ''}
      </p>
      {diagnostics.warnings.map((warning) => (
        <p
          key={warning}
          className="mt-1 text-[10px] leading-snug text-amber-400/80"
        >
          {warning}
        </p>
      ))}
    </div>
  )
}

interface ToolsGridProps {
  status: ServerToolsStatus
  prefixConfigured: boolean
  dgvoodooNeedsInstall: boolean
  opening: ToolKind | null
  installingDgVoodoo: boolean
  uninstallingDgVoodoo: boolean
  busy: boolean
  onOpen: (tool: ToolKind) => void
  onInstallDgVoodoo: () => void
  onUninstallDgVoodoo: () => void
}

interface SimpleToolConfig {
  kind: ToolKind
  label: string
  tool: ToolInfo
}

const SIMPLE_TOOLS: (status: ServerToolsStatus) => SimpleToolConfig[] = (
  status,
) => [
  { kind: 'opensetup', label: 'OpenSetup', tool: status.openSetup },
  { kind: 'patcher', label: 'Patcher', tool: status.patcher },
]

function toolDetail(tool: ToolInfo): string {
  return tool.label ?? (tool.found ? 'OK' : '—')
}

function CompactToolCard({
  label,
  detail,
  dotOk,
  actionLabel,
  actionBusy,
  actionDisabled,
  onAction,
  secondaryLabel,
  secondaryBusy,
  onSecondary,
}: {
  label: string
  detail: string
  dotOk: boolean
  actionLabel?: string
  actionBusy?: boolean
  actionDisabled?: boolean
  onAction?: () => void
  secondaryLabel?: string
  secondaryBusy?: boolean
  onSecondary?: () => void
}) {
  const btnClass = buttonClasses('secondary', 'xs')

  return (
    <div className="rounded-lg border border-white/[0.04] bg-zinc-950/40 px-2.5 py-2 flex flex-col gap-1.5 min-w-0">
      <div className="flex items-center gap-1.5 min-w-0">
        <StatusDot status={dotOk ? 'ok' : 'neutral'} />
        <span className="text-[11px] text-zinc-300 font-medium shrink-0">
          {label}
        </span>
        <span
          className="text-[10px] text-zinc-600 truncate font-mono"
          title={detail}
        >
          {detail}
        </span>
      </div>
      {(onAction || onSecondary) && (
        <div className="flex gap-1">
          {onSecondary && secondaryLabel && (
            <button
              type="button"
              onClick={onSecondary}
              disabled={secondaryBusy}
              className={btnClass}
            >
              {secondaryBusy ? '...' : secondaryLabel}
            </button>
          )}
          {onAction && actionLabel && (
            <button
              type="button"
              onClick={onAction}
              disabled={actionDisabled || actionBusy}
              className={btnClass}
            >
              {actionBusy ? '...' : actionLabel}
            </button>
          )}
        </div>
      )}
    </div>
  )
}

function ToolsGrid({
  status,
  prefixConfigured,
  dgvoodooNeedsInstall,
  opening,
  installingDgVoodoo,
  uninstallingDgVoodoo,
  busy,
  onOpen,
  onInstallDgVoodoo,
  onUninstallDgVoodoo,
}: ToolsGridProps) {
  const dg = status.dgvoodoo

  return (
    <div className="grid grid-cols-3 gap-2">
      {SIMPLE_TOOLS(status).map(({ kind, label, tool }) => (
        <CompactToolCard
          key={kind}
          label={label}
          detail={toolDetail(tool)}
          dotOk={tool.found}
          onAction={tool.found ? () => onOpen(kind) : undefined}
          actionLabel="Abrir"
          actionBusy={busy || opening === kind}
          actionDisabled={!tool.found || !prefixConfigured}
        />
      ))}
      <CompactToolCard
        label="dgVoodoo"
        detail={dg.configured ? 'conf OK' : '—'}
        dotOk={dg.configured}
        onAction={
          dgvoodooNeedsInstall
            ? onInstallDgVoodoo
            : dg.cpl.found
              ? () => onOpen('dgvoodoo')
              : undefined
        }
        actionLabel={
          dgvoodooNeedsInstall
            ? dg.canUninstall
              ? 'Reparar'
              : 'Instalar'
            : 'Config'
        }
        actionBusy={
          busy ||
          (dgvoodooNeedsInstall ? installingDgVoodoo : opening === 'dgvoodoo')
        }
        actionDisabled={
          !dgvoodooNeedsInstall && dg.cpl.found && !prefixConfigured
        }
        onSecondary={dg.canUninstall ? onUninstallDgVoodoo : undefined}
        secondaryLabel="Quitar"
        secondaryBusy={busy || uninstallingDgVoodoo}
      />
    </div>
  )
}
