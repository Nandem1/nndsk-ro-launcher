import { useUiModeStore } from '../../app/uiMode.store'
import { useState } from 'react'
import { ChevronDown } from 'lucide-react'
import { IconButton } from '../../shared/ui/Button'
import { Panel, resolveToolTone } from '../../shared/ui/Panel'
import { ToggleSwitch } from '../../shared/ui/ToggleSwitch'
import { useLauncherStore } from '../launcher/launcher.store'
import { useSelectedServer } from '../servers/useSelectedServer'
import { AutobuffRulesEditor } from './AutobuffRulesEditor'
import { useAutobuff } from './useAutobuff'
import { DataText } from '../../shared/ui/DataText'
import {
  memoryAccessAction,
  memoryAccessLabel,
  memoryAccessUsable,
} from '../autopot/memoryAccess.logic'

export function AutobuffPanel() {
  const server = useSelectedServer()
  const { config, status, busy, isRunning, error, setEnabled, updateField } =
    useAutobuff(server)
  const [catalogOpen, setCatalogOpen] = useState(
    () => config.rules.length === 0,
  )
  const launching = useLauncherStore((state) => state.status === 'launching')
  const multipleClients = useLauncherStore((state) => state.clients.length > 1)
  const hero = useUiModeStore((state) => state.mode === 'ingame')
  const clientMemoryAccess = useLauncherStore((s) =>
    s.clients.length === 1 ? s.clients[0].memoryAccess : null,
  )
  const available = isRunning && !!server
  const hasEnabledRule = config.rules.some((rule) => rule.enabled)
  const effectiveMemoryAccess = status.memoryAccess ?? clientMemoryAccess
  const memoryReady = memoryAccessUsable(effectiveMemoryAccess)
  const memoryAction = memoryAccessAction(effectiveMemoryAccess)
  const tone = resolveToolTone(
    available && memoryReady,
    config.enabled &&
      status.active &&
      memoryAccessUsable(effectiveMemoryAccess),
    !!error || (available && !memoryReady),
  )

  return (
    <Panel
      title="AutoBuff"
      size={hero ? 'hero' : 'compact'}
      tone={tone}
      className="h-full w-full min-h-0 overflow-hidden"
      action={
        <IconButton
          label={catalogOpen ? 'Ocultar catálogo' : 'Mostrar catálogo'}
          size="xs"
          aria-expanded={catalogOpen}
          onClick={() => setCatalogOpen(!catalogOpen)}
        >
          <ChevronDown
            className={`h-3.5 w-3.5 ${catalogOpen ? 'rotate-180' : ''}`}
            aria-hidden
          />
        </IconButton>
      }
    >
      <div className="flex min-h-0 flex-1 flex-col gap-2">
        <div className="flex shrink-0 items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-semibold text-ink">
              {status.lastAppliedRule ?? 'Sin buffs aplicados'}
            </p>
            <p
              className={`text-caption ${launching ? 'text-muted animate-pulse-dot' : 'text-muted'}`}
            >
              {!server ? (
                'Selecciona un servidor'
              ) : launching ? (
                'Iniciando juego...'
              ) : multipleClients ? (
                'No disponible con varios clientes'
              ) : !isRunning ? (
                'Inicia el juego'
              ) : (
                <DataText>{`${status.activeStatuses} estados detectados`}</DataText>
              )}
            </p>
          </div>
          <ToggleSwitch
            checked={
              config.enabled && available && memoryReady && hasEnabledRule
            }
            disabled={!available || !memoryReady || busy || !hasEnabledRule}
            onChange={(enabled) => void setEnabled(enabled)}
            tone="ok"
          />
        </div>

        <div
          data-design-panel-scroll
          className="min-h-0 flex-1 overflow-y-auto overscroll-contain"
        >
          <AutobuffRulesEditor
            rules={config.rules}
            catalogOpen={catalogOpen}
            disabled={!server || busy}
            onChange={(rules) => void updateField({ rules })}
          />
        </div>

        <p className="shrink-0 text-caption leading-snug text-muted">
          Activa cada buff y asigna la tecla donde lo tienes configurado en el
          juego.
        </p>
        <p className="shrink-0 text-caption leading-snug min-h-[calc(1em*1.375)]">
          {error && available ? (
            <span className="text-bad">{error}</span>
          ) : available && effectiveMemoryAccess && !memoryReady ? (
            <span className="inline-block notice-warn text-muted">
              {memoryAccessLabel(effectiveMemoryAccess)}
              <span className="text-muted">
                {memoryAction ? ` ${memoryAction}` : ''}
              </span>
            </span>
          ) : null}
        </p>
      </div>
    </Panel>
  )
}
