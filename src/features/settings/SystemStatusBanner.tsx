import { audioDriverLabel } from '../../shared/audio'
import { StatusDot, type DotStatus } from '../../shared/ui/StatusDot'
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
        <p className="text-[10px] text-zinc-500 leading-snug pl-4 truncate">{hint}</p>
      )}
    </div>
  )
}

export function SystemStatusBanner() {
  const audioStatus = useSettingsStore((s) => s.audioStatus)
  const autopotInputStatus = useSettingsStore((s) => s.autopotInputStatus)

  if (!audioStatus) return null

  const audioDot: DotStatus = !audioStatus.audioOk
    ? 'error'
    : audioStatus.audioWarning
      ? 'warning'
      : 'ok'

  const audioLabel = `Audio · ${audioDriverLabel(audioStatus.audioDriver)}${
    !audioStatus.audioOk ? ' (no disponible)' : ''
  }`

  const autopotWarn =
    autopotInputStatus != null && !autopotInputStatus.autopotInputOk

  const hasIssue =
    audioDot !== 'ok' || autopotWarn

  return (
    <div
      className={`rounded-xl border px-3 py-2 shrink-0 space-y-1 ${
        hasIssue
          ? 'border-amber-500/25 bg-amber-500/5'
          : 'border-zinc-800/80 bg-zinc-900/40'
      }`}
    >
      <StatusLine
        dotStatus={audioDot}
        label={audioLabel}
        hint={audioStatus.audioWarning}
      />
      {autopotWarn && (
        <StatusLine
          dotStatus="warning"
          label="AutoPot · input no disponible"
          hint={autopotInputStatus.autopotInputWarning}
        />
      )}
    </div>
  )
}
