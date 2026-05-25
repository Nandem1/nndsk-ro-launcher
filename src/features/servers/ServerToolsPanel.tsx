import type {
  ServerToolsStatus,
  ToolInfo,
  ToolKind,
} from '../../shared/types'
import { Panel } from '../../shared/ui/Panel'
import { ToolRow } from './ToolRow'
import { useSelectedServer } from './useSelectedServer'
import { useServerTools } from './useServerTools'

export function ServerToolsPanel() {
  const server = useSelectedServer()
  const {
    status,
    loading,
    error,
    opening,
    installingDgVoodoo,
    uninstallingDgVoodoo,
    refresh,
    handleInstallDgVoodoo,
    handleUninstallDgVoodoo,
    handleOpen,
  } = useServerTools(server)

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
        <div className="max-h-[220px] overflow-y-auto -mx-1 px-1">
          <ToolsList
            status={status}
            dgvoodooNeedsInstall={!!dgvoodooNeedsInstall}
            opening={opening}
            installingDgVoodoo={installingDgVoodoo}
            uninstallingDgVoodoo={uninstallingDgVoodoo}
            onOpen={handleOpen}
            onInstallDgVoodoo={handleInstallDgVoodoo}
            onUninstallDgVoodoo={handleUninstallDgVoodoo}
          />
        </div>
      )}

      {loading && !status && (
        <p className="text-xs text-zinc-600 py-2 text-center">Escaneando carpeta...</p>
      )}
    </Panel>
  )
}

interface ToolsListProps {
  status: ServerToolsStatus
  dgvoodooNeedsInstall: boolean
  opening: ToolKind | null
  installingDgVoodoo: boolean
  uninstallingDgVoodoo: boolean
  onOpen: (tool: ToolKind) => void
  onInstallDgVoodoo: () => void
  onUninstallDgVoodoo: () => void
}

interface SimpleToolConfig {
  kind: ToolKind
  label: string
  tool: ToolInfo
}

const SIMPLE_TOOLS: (status: ServerToolsStatus) => SimpleToolConfig[] = (status) => [
  { kind: 'opensetup', label: 'OpenSetup', tool: status.openSetup },
  { kind: 'patcher', label: 'Patcher', tool: status.patcher },
]

function toolDetail(tool: ToolInfo): string {
  return tool.label ?? (tool.found ? 'Detectado' : 'No encontrado')
}

function ToolsList({
  status,
  dgvoodooNeedsInstall,
  opening,
  installingDgVoodoo,
  uninstallingDgVoodoo,
  onOpen,
  onInstallDgVoodoo,
  onUninstallDgVoodoo,
}: ToolsListProps) {
  return (
    <div>
      {SIMPLE_TOOLS(status).map(({ kind, label, tool }) => (
        <ToolRow
          key={kind}
          label={label}
          dotStatus={tool.found ? 'ok' : 'neutral'}
          detail={toolDetail(tool)}
          onAction={tool.found ? () => onOpen(kind) : undefined}
          actionLabel="Abrir"
          actionBusy={opening === kind}
          actionDisabled={!tool.found}
        />
      ))}
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
            ? onInstallDgVoodoo
            : status.dgvoodoo.cpl.found
              ? () => onOpen('dgvoodoo')
              : undefined
        }
        actionLabel={dgvoodooNeedsInstall ? 'Instalar' : 'Configurar'}
        actionBusy={dgvoodooNeedsInstall ? installingDgVoodoo : opening === 'dgvoodoo'}
        onSecondary={
          status.dgvoodoo.canUninstall ? onUninstallDgVoodoo : undefined
        }
        secondaryLabel="Desinstalar"
        secondaryBusy={uninstallingDgVoodoo}
        secondaryDanger
      />
    </div>
  )
}
