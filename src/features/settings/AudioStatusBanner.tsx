import { invoke } from '@tauri-apps/api/core'
import { useCallback, useEffect, useState } from 'react'
import { useSettingsStore } from './settings.store'

interface DependencyStatus {
  audioOk: boolean
  audioDriver: string
  audioWarning: string | null
}

export function AudioStatusBanner() {
  const selectedRunner = useSettingsStore((s) => s.selectedRunner)
  const [status, setStatus] = useState<DependencyStatus | null>(null)

  const refresh = useCallback(async () => {
    if (!selectedRunner) {
      setStatus(null)
      return
    }

    try {
      const deps = await invoke<{
        audioOk: boolean
        audioDriver: string
        audioWarning?: string
      }>('check_dependencies', { runner: selectedRunner })

      setStatus({
        audioOk: deps.audioOk,
        audioDriver: deps.audioDriver,
        audioWarning: deps.audioWarning ?? null,
      })
    } catch {
      setStatus(null)
    }
  }, [selectedRunner])

  useEffect(() => {
    refresh()
  }, [refresh])

  if (!status) return null

  const driverLabel =
    status.audioDriver === 'pulse'
      ? 'PulseAudio'
      : status.audioDriver === 'alsa'
        ? 'ALSA'
        : 'sin driver'

  if (status.audioOk && !status.audioWarning) {
    return (
      <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 px-3 py-2.5 shrink-0">
        <div className="flex items-center gap-2">
          <span className="inline-block w-2 h-2 rounded-full bg-emerald-500 shadow-[0_0_6px_rgba(16,185,129,0.5)] shrink-0" />
          <p className="text-xs text-emerald-400/90">
            Audio · <span className="font-medium text-emerald-300">{driverLabel}</span>
          </p>
        </div>
      </div>
    )
  }

  return (
    <div className="rounded-xl border border-amber-500/30 bg-amber-500/5 px-3 py-2.5 shrink-0">
      <div className="flex items-center gap-2">
        <span className="inline-block w-2 h-2 rounded-full bg-amber-500 shrink-0" />
        <p className="text-xs font-medium text-amber-400">
          Audio · {driverLabel}
          {!status.audioOk && ' (no disponible)'}
        </p>
      </div>
      {status.audioWarning && (
        <p className="mt-1.5 text-xs text-zinc-400 leading-relaxed pl-4">{status.audioWarning}</p>
      )}
    </div>
  )
}
