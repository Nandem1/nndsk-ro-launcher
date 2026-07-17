import { audioStatusLabel } from '../../shared/audio'
import { Panel } from '../../shared/ui/Panel'
import { StatusDot, type DotStatus } from '../../shared/ui/StatusDot'
import {
  advancedHasIssue,
  resolveAudioDotStatus,
  resolveDotStatus,
} from './advanced.logic'
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
  const advancedStatus = useSettingsStore((s) => s.advancedStatus)

  if (!advancedStatus) return null

  const hasIssue = advancedHasIssue(advancedStatus)

  const audioDot = resolveAudioDotStatus(
    advancedStatus.audioOk,
    advancedStatus.audioWarning,
  )
  const audioLabel = `Audio · ${audioStatusLabel(
    advancedStatus.audioDriver,
    advancedStatus.audioStack,
  )}${!advancedStatus.audioOk ? ' (no disponible)' : ''}`

  const lines = [
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
        ? 'Prefix · configurado'
        : 'Prefix · sin configurar',
      hint: advancedStatus.prefixWarning,
    },
    {
      key: 'dxvk',
      dot: resolveDotStatus(advancedStatus.dxvkOk, advancedStatus.dxvkWarning),
      label: advancedStatus.dxvkOk ? 'DXVK · instalado' : 'DXVK · pendiente',
      hint: advancedStatus.dxvkWarning,
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
      </div>
    </Panel>
  )
}
