import { useEffect, useState } from 'react'
import { save } from '@tauri-apps/plugin-dialog'
import { audioStatusLabel } from '../../shared/audio'
import { api } from '../../shared/api'
import { resolveRunner } from '../../shared/resolveRunner'
import { Panel } from '../../shared/ui/Panel'
import { StatusDot, type DotStatus } from '../../shared/ui/StatusDot'
import { useSelectedServer } from '../servers/useSelectedServer'
import {
  advancedHasIssue,
  compatibilityLine,
  observationsLabel,
  dxvkHintFromDeps,
  resolveAudioDotStatus,
  resolveDotStatus,
} from './advanced.logic'
import { useCurrentAdvancedStatus } from './useSelectedRuntimeStatus'
import { useSettingsStore } from './settings.store'

function StatusLine({
  dotStatus,
  label,
  hint,
}: {
  dotStatus: DotStatus
  label: string
  hint?: string | null
}) {
  return (
    <div className="min-w-0" title={hint ?? undefined}>
      <div className="flex items-center gap-2 min-w-0">
        <StatusDot status={dotStatus} />
        <p className="text-[11px] text-zinc-400 truncate">{label}</p>
      </div>
      {hint && (
        <p className="text-[10px] text-zinc-500 leading-snug pl-4 truncate">
          {hint}
        </p>
      )}
    </div>
  )
}

export function AdvancedSettings() {
  const [observationCount, setObservationCount] = useState(0)
  const advancedStatus = useCurrentAdvancedStatus()
  const runners = useSettingsStore((state) => state.runners)
  const selectedRunner = useSettingsStore((state) => state.selectedRunner)
  const server = useSelectedServer()

  useEffect(() => {
    void api.listRuntimeObservations().then((rows) => {
      setObservationCount(rows.length)
    })
  }, [advancedStatus])

  if (!advancedStatus) return null

  const refreshObservations = () => {
    void api.listRuntimeObservations().then((rows) => {
      setObservationCount(rows.length)
    })
  }

  const exportObservations = async () => {
    const dest = await save({
      defaultPath: 'ro-launcher-observations.json',
      filters: [{ name: 'JSON', extensions: ['json'] }],
    })
    if (!dest) return
    await api.exportRuntimeObservations(dest)
    refreshObservations()
  }

  const deleteObservations = async () => {
    if (!window.confirm('¿Borrar todas las observaciones locales de runtime?'))
      return
    await api.deleteRuntimeObservations()
    refreshObservations()
  }

  const effectiveRunner = server
    ? resolveRunner(server, selectedRunner)
    : selectedRunner || null
  const effectiveRunnerName =
    runners.find((runner) => runner.path === effectiveRunner)?.name ??
    advancedStatus.runnerKind
  const runnerSource = server?.runner?.trim()
    ? 'Propio del servidor'
    : 'Predeterminado global'

  const hasIssue = advancedHasIssue(advancedStatus)

  const audioDot = resolveAudioDotStatus(
    advancedStatus.audioOk,
    advancedStatus.audioWarning,
  )
  const audioLabel = `Audio · ${audioStatusLabel(
    advancedStatus.audioDriver,
    advancedStatus.audioStack,
  )}${!advancedStatus.audioOk ? ' (no disponible)' : ''}`

  const compatibilityStatusLine = advancedStatus.compatibility
    ? compatibilityLine(advancedStatus.compatibility)
    : null

  const lines = [
    {
      key: 'runner',
      dot: resolveDotStatus(
        advancedStatus.runnerOk,
        advancedStatus.runnerWarning,
      ),
      label: `Runner · ${effectiveRunnerName}`,
      hint:
        advancedStatus.runnerWarning ??
        `${runnerSource}${effectiveRunner ? ` · ${effectiveRunner}` : ''}`,
    },
    ...(compatibilityStatusLine
      ? [
          {
            key: 'compatibility',
            dot: compatibilityStatusLine.dot,
            label: compatibilityStatusLine.label,
            hint: compatibilityStatusLine.hint,
          },
        ]
      : []),
    {
      key: 'audio',
      dot: audioDot,
      label: audioLabel,
      hint: advancedStatus.audioWarning,
    },
    {
      key: 'prefix',
      dot: resolveDotStatus(
        advancedStatus.prefixOk,
        advancedStatus.prefixWarning,
      ),
      label: advancedStatus.prefixOk
        ? `Entorno ${advancedStatus.prefixScope} · listo`
        : `Entorno ${advancedStatus.prefixScope} · pendiente`,
      hint:
        advancedStatus.prefixWarning ??
        `${advancedStatus.prefixManaged ? 'Administrado' : 'Externo'} · ${advancedStatus.prefixPath}`,
    },
    {
      key: 'dxvk',
      dot: resolveDotStatus(advancedStatus.dxvkOk, advancedStatus.dxvkWarning),
      label: advancedStatus.dxvk ? 'DXVK · instalado' : 'DXVK · pendiente',
      hint: dxvkHintFromDeps(advancedStatus) ?? advancedStatus.dxvkWarning,
    },
    {
      key: 'input-group',
      dot: resolveDotStatus(
        advancedStatus.inputGroupOk,
        advancedStatus.inputGroupWarning,
      ),
      label: advancedStatus.inputGroupOk
        ? advancedStatus.inputGroupWarning
          ? 'Permisos input · parcial'
          : 'Permisos input · OK'
        : 'Permisos input · grupo input',
      hint: advancedStatus.inputGroupWarning,
    },
    {
      key: 'observations',
      dot: 'ok' as const,
      label: observationsLabel(observationCount),
      hint: 'Registros locales de ejecución (sin subir a red)',
    },
    {
      key: 'uinput',
      dot: resolveDotStatus(
        advancedStatus.uinputInputOk,
        advancedStatus.uinputInputWarning,
      ),
      label: advancedStatus.uinputInputOk
        ? 'Input de combate · uinput OK'
        : 'Input de combate · uinput no disponible',
      hint: advancedStatus.uinputInputWarning,
    },
  ]

  return (
    <Panel
      title="Avanzado"
      compact
      tone={hasIssue ? 'warning' : 'neutral'}
      className="shrink-0"
    >
      <div
        className={`space-y-1 rounded-lg ${hasIssue ? 'bg-amber-500/5 px-2 py-1.5 -mx-0.5' : ''}`}
      >
        {lines.map((line) => (
          <StatusLine
            key={line.key}
            dotStatus={line.dot}
            label={line.label}
            hint={line.hint}
          />
        ))}
        <div className="flex gap-2 pt-1 pl-4">
          <button
            type="button"
            className="text-[10px] text-zinc-400 hover:text-zinc-200"
            onClick={() => void exportObservations()}
          >
            Exportar observaciones
          </button>
          <button
            type="button"
            className="text-[10px] text-zinc-400 hover:text-zinc-200"
            onClick={() => void deleteObservations()}
          >
            Borrar
          </button>
        </div>
      </div>
    </Panel>
  )
}
