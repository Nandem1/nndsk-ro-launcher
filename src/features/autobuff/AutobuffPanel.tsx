import { Sparkles } from 'lucide-react'
import { useUiModeStore } from '../../app/uiMode.store'
import { Panel, resolveToolTone } from '../../shared/ui/Panel'
import { ToggleSwitch } from '../../shared/ui/ToggleSwitch'
import { useLauncherStore } from '../launcher/launcher.store'
import { useSelectedServer } from '../servers/useSelectedServer'
import { AutobuffRulesEditor } from './AutobuffRulesEditor'
import { useAutobuff } from './useAutobuff'

export function AutobuffPanel() {
  const server = useSelectedServer()
  const { config, status, busy, isRunning, error, setEnabled, updateField } =
    useAutobuff(server)
  const launching = useLauncherStore((state) => state.status === 'launching')
  const hero = useUiModeStore((state) => state.mode === 'ingame')
  const available = isRunning && !!server
  const hasEnabledRule = config.rules.some((rule) => rule.enabled)
  const tone = resolveToolTone(
    available,
    config.enabled && status.active,
    !!error,
  )

  return (
    <Panel
      title="AutoBuff"
      compact
      hero={hero}
      tone={tone}
      className="h-full w-full"
      leading={
        <Sparkles className="w-3 h-3 text-zinc-600 shrink-0" aria-hidden />
      }
    >
      <div className="flex min-h-0 flex-1 flex-col gap-2">
        <div className="flex shrink-0 items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-semibold text-zinc-100">
              {status.lastAppliedRule ?? 'Sin buffs aplicados'}
            </p>
            <p
              className={`text-[10px] ${launching ? 'text-zinc-500 animate-pulse-dot' : 'text-zinc-600'}`}
            >
              {!server
                ? 'Selecciona un servidor'
                : launching
                  ? 'Iniciando juego...'
                  : !isRunning
                    ? 'Inicia el juego'
                    : `${status.activeStatuses} estados detectados`}
            </p>
          </div>
          <ToggleSwitch
            checked={config.enabled && available && hasEnabledRule}
            disabled={!available || busy || !hasEnabledRule}
            onChange={(enabled) => void setEnabled(enabled)}
            tone="emerald"
          />
        </div>

        <AutobuffRulesEditor
          rules={config.rules}
          disabled={!server || busy}
          onChange={(rules) => void updateField({ rules })}
        />

        <p className="shrink-0 text-[10px] leading-snug text-zinc-600">
          Activa cada buff y asigna la tecla donde lo tienes configurado en el
          juego.
        </p>
        <p className="shrink-0 text-[10px] leading-snug min-h-[calc(1em*1.375)]">
          {error && available ? (
            <span className="text-red-400/90">{error}</span>
          ) : null}
        </p>
      </div>
    </Panel>
  )
}
