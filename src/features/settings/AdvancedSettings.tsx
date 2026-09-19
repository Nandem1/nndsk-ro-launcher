import { useEffect, useState } from 'react'
import { open, save } from '@tauri-apps/plugin-dialog'
import { audioStatusLabel } from '../../shared/audio'
import { api } from '../../shared/api'
import { resolveRunner } from '../../shared/resolveRunner'
import { Panel } from '../../shared/ui/Panel'
import { StatusDot, type DotStatus } from '../../shared/ui/StatusDot'
import { useSelectedServer } from '../servers/useSelectedServer'
import type {
  BenchmarkComparison,
  BenchmarkRunSummary,
} from '../../shared/types'
import {
  advancedHasIssue,
  benchmarksLabel,
  comparisonSummary,
  compatibilityLine,
  observationsLabel,
  dxvkHintFromDeps,
  formatFrametimeLine,
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
  const [benchmarkCount, setBenchmarkCount] = useState(0)
  const [benchmarkRuns, setBenchmarkRuns] = useState<BenchmarkRunSummary[]>([])
  const [runningClientId, setRunningClientId] = useState('')
  const [activeRunId, setActiveRunId] = useState<string | null>(null)
  const [arm, setArm] = useState<'a' | 'b'>('a')
  const [sceneId, setSceneId] = useState('payon-town')
  const [loadDescriptor, setLoadDescriptor] = useState('idle')
  const [width, setWidth] = useState(1920)
  const [height, setHeight] = useState(1080)
  const [fullscreen, setFullscreen] = useState(false)
  const [warmupSeconds, setWarmupSeconds] = useState(60)
  const [captureSeconds, setCaptureSeconds] = useState(30)
  const [compareLeft, setCompareLeft] = useState<Set<string>>(new Set())
  const [compareRight, setCompareRight] = useState<Set<string>>(new Set())
  const [comparison, setComparison] = useState<BenchmarkComparison | null>(null)
  const [benchmarkBusy, setBenchmarkBusy] = useState(false)
  const advancedStatus = useCurrentAdvancedStatus()
  const runners = useSettingsStore((state) => state.runners)
  const selectedRunner = useSettingsStore((state) => state.selectedRunner)
  const server = useSelectedServer()

  const refreshBenchmarks = () => {
    void api.listRuntimeBenchmarks().then((rows) => {
      setBenchmarkRuns(rows)
      setBenchmarkCount(rows.length)
    })
  }

  useEffect(() => {
    void api.listRuntimeObservations().then((rows) => {
      setObservationCount(rows.length)
    })
    refreshBenchmarks()
    void api.listGameClients().then((clients) => {
      if (!server) return
      const running = clients.find(
        (c) => c.status === 'running' && c.serverId === server.id,
      )
      setRunningClientId(running?.clientId ?? '')
    })
  }, [advancedStatus, server])

  const effectiveRunner = server
    ? resolveRunner(server, selectedRunner)
    : selectedRunner || null

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

  const attachBenchmark = async () => {
    if (!server || !runningClientId) return
    setBenchmarkBusy(true)
    try {
      const summary = await api.startRuntimeBenchmarkRun({
        clientId: runningClientId,
        server,
        runner: effectiveRunner,
        arm,
        sceneId,
        loadDescriptor,
        resolution: { width, height, fullscreen },
        warmupSeconds,
        captureSeconds,
      })
      setActiveRunId(summary.runId)
      refreshBenchmarks()
    } finally {
      setBenchmarkBusy(false)
    }
  }

  const runId = activeRunId

  const beginCapture = async () => {
    if (!runId) return
    setBenchmarkBusy(true)
    try {
      await api.beginRuntimeBenchmarkCapture(runId)
      refreshBenchmarks()
    } finally {
      setBenchmarkBusy(false)
    }
  }

  const finishCapture = async () => {
    if (!runId) return
    setBenchmarkBusy(true)
    try {
      await api.finishRuntimeBenchmarkCapture(runId)
      refreshBenchmarks()
    } finally {
      setBenchmarkBusy(false)
    }
  }

  const importCsv = async () => {
    if (!runId) return
    const csvPath = await open({
      filters: [{ name: 'CSV', extensions: ['csv'] }],
    })
    if (!csvPath || typeof csvPath !== 'string') return
    setBenchmarkBusy(true)
    try {
      await api.importRuntimeBenchmarkSamples(runId, csvPath)
      refreshBenchmarks()
    } finally {
      setBenchmarkBusy(false)
    }
  }

  const setVisual = async (status: string) => {
    if (!runId) return
    setBenchmarkBusy(true)
    try {
      await api.setRuntimeBenchmarkVisualCheck(runId, status)
      refreshBenchmarks()
    } finally {
      setBenchmarkBusy(false)
    }
  }

  const runCompare = async () => {
    setBenchmarkBusy(true)
    try {
      const result = await api.compareRuntimeBenchmarks(
        [...compareLeft],
        [...compareRight],
      )
      setComparison(result)
    } finally {
      setBenchmarkBusy(false)
    }
  }

  const exportBenchmarks = async () => {
    const dest = await save({
      defaultPath: 'ro-launcher-benchmarks.json',
      filters: [{ name: 'JSON', extensions: ['json'] }],
    })
    if (!dest) return
    const runIds = benchmarkRuns.map((r) => r.runId)
    await api.exportRuntimeBenchmarks(dest, runIds)
    refreshBenchmarks()
  }

  const exportComparison = async () => {
    const dest = await save({
      defaultPath: 'ro-launcher-benchmark-comparison.json',
      filters: [{ name: 'JSON', extensions: ['json'] }],
    })
    if (!dest) return
    await api.exportRuntimeBenchmarkComparison(
      dest,
      [...compareLeft],
      [...compareRight],
    )
  }

  const deleteBenchmarks = async () => {
    if (!window.confirm('¿Borrar todos los benchmarks locales A/B?')) return
    await api.deleteRuntimeBenchmarks()
    setActiveRunId(null)
    setComparison(null)
    setCompareLeft(new Set())
    setCompareRight(new Set())
    refreshBenchmarks()
  }

  const toggleCompare = (
    side: 'left' | 'right',
    runIdToToggle: string,
    checked: boolean,
  ) => {
    const setter = side === 'left' ? setCompareLeft : setCompareRight
    setter((prev) => {
      const next = new Set(prev)
      if (checked) next.add(runIdToToggle)
      else next.delete(runIdToToggle)
      return next
    })
  }

  const comparisonLine = comparison ? comparisonSummary(comparison) : null

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
      key: 'benchmarks',
      dot: 'ok' as const,
      label: benchmarksLabel(benchmarkCount),
      hint: 'Medición A/B local opt-in; no altera el lanzamiento ni Gepard',
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
        <div className="mt-2 space-y-1.5 pl-4 border-t border-zinc-800/80 pt-2">
          <p className="text-[10px] text-zinc-500">
            Cliente en ejecución:{' '}
            {runningClientId ? runningClientId.slice(0, 8) : 'ninguno'}
            {activeRunId ? ` · run ${activeRunId.slice(0, 8)}` : ''}
          </p>
          <div className="flex flex-wrap gap-1.5 text-[10px]">
            <select
              className="bg-zinc-900 border border-zinc-700 rounded px-1 py-0.5"
              value={arm}
              onChange={(e) => setArm(e.target.value as 'a' | 'b')}
            >
              <option value="a">Brazo A</option>
              <option value="b">Brazo B</option>
            </select>
            <input
              className="w-24 bg-zinc-900 border border-zinc-700 rounded px-1 py-0.5"
              value={sceneId}
              onChange={(e) => setSceneId(e.target.value)}
              placeholder="scene"
            />
            <input
              className="w-16 bg-zinc-900 border border-zinc-700 rounded px-1 py-0.5"
              value={loadDescriptor}
              onChange={(e) => setLoadDescriptor(e.target.value)}
              placeholder="load"
            />
            <input
              type="number"
              className="w-14 bg-zinc-900 border border-zinc-700 rounded px-1 py-0.5"
              value={width}
              onChange={(e) => setWidth(Number(e.target.value))}
            />
            <input
              type="number"
              className="w-14 bg-zinc-900 border border-zinc-700 rounded px-1 py-0.5"
              value={height}
              onChange={(e) => setHeight(Number(e.target.value))}
            />
            <label className="flex items-center gap-1 text-zinc-400">
              <input
                type="checkbox"
                checked={fullscreen}
                onChange={(e) => setFullscreen(e.target.checked)}
              />
              FS
            </label>
            <input
              type="number"
              className="w-12 bg-zinc-900 border border-zinc-700 rounded px-1 py-0.5"
              value={warmupSeconds}
              onChange={(e) => setWarmupSeconds(Number(e.target.value))}
              title="warmup s"
            />
            <input
              type="number"
              className="w-12 bg-zinc-900 border border-zinc-700 rounded px-1 py-0.5"
              value={captureSeconds}
              onChange={(e) => setCaptureSeconds(Number(e.target.value))}
              title="capture s"
            />
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              disabled={benchmarkBusy || !server || !runningClientId}
              className="text-[10px] text-zinc-400 hover:text-zinc-200 disabled:opacity-40"
              onClick={() => void attachBenchmark()}
            >
              Adjuntar
            </button>
            <button
              type="button"
              disabled={benchmarkBusy || !runId}
              className="text-[10px] text-zinc-400 hover:text-zinc-200 disabled:opacity-40"
              onClick={() => void beginCapture()}
            >
              Iniciar captura
            </button>
            <button
              type="button"
              disabled={benchmarkBusy || !runId}
              className="text-[10px] text-zinc-400 hover:text-zinc-200 disabled:opacity-40"
              onClick={() => void finishCapture()}
            >
              Terminar captura
            </button>
            <button
              type="button"
              disabled={benchmarkBusy || !runId}
              className="text-[10px] text-zinc-400 hover:text-zinc-200 disabled:opacity-40"
              onClick={() => void importCsv()}
            >
              Importar CSV
            </button>
            <button
              type="button"
              disabled={benchmarkBusy || !runId}
              className="text-[10px] text-zinc-400 hover:text-zinc-200 disabled:opacity-40"
              onClick={() => void setVisual('passed')}
            >
              Visual OK
            </button>
            <button
              type="button"
              disabled={benchmarkBusy || !runId}
              className="text-[10px] text-zinc-400 hover:text-zinc-200 disabled:opacity-40"
              onClick={() => void setVisual('failed')}
            >
              Visual fallo
            </button>
            <button
              type="button"
              disabled={benchmarkBusy || !runId}
              className="text-[10px] text-zinc-400 hover:text-zinc-200 disabled:opacity-40"
              onClick={() => void setVisual('skipped')}
            >
              Visual omitir
            </button>
          </div>
          {benchmarkRuns.length > 0 && (
            <div className="max-h-24 overflow-y-auto space-y-0.5 text-[10px] text-zinc-500">
              {benchmarkRuns.map((run) => (
                <div key={run.runId} className="flex items-center gap-2">
                  <label className="flex items-center gap-0.5">
                    <input
                      type="checkbox"
                      checked={compareLeft.has(run.runId)}
                      onChange={(e) =>
                        toggleCompare('left', run.runId, e.target.checked)
                      }
                    />
                    Izq
                  </label>
                  <label className="flex items-center gap-0.5">
                    <input
                      type="checkbox"
                      checked={compareRight.has(run.runId)}
                      onChange={(e) =>
                        toggleCompare('right', run.runId, e.target.checked)
                      }
                    />
                    Der
                  </label>
                  <span className="truncate">
                    {run.arm} · {run.sceneId} · {run.recordState}
                  </span>
                </div>
              ))}
            </div>
          )}
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              disabled={
                benchmarkBusy ||
                compareLeft.size === 0 ||
                compareRight.size === 0
              }
              className="text-[10px] text-zinc-400 hover:text-zinc-200 disabled:opacity-40"
              onClick={() => void runCompare()}
            >
              Comparar
            </button>
            <button
              type="button"
              className="text-[10px] text-zinc-400 hover:text-zinc-200"
              onClick={() => void exportBenchmarks()}
            >
              Exportar benchmarks
            </button>
            <button
              type="button"
              disabled={
                benchmarkBusy ||
                compareLeft.size === 0 ||
                compareRight.size === 0
              }
              className="text-[10px] text-zinc-400 hover:text-zinc-200 disabled:opacity-40"
              onClick={() => void exportComparison()}
            >
              Exportar comparación
            </button>
            <button
              type="button"
              className="text-[10px] text-zinc-400 hover:text-zinc-200"
              onClick={() => void deleteBenchmarks()}
            >
              Borrar benchmarks
            </button>
          </div>
          {comparisonLine && (
            <p
              className="text-[10px] text-zinc-400"
              title={comparisonLine.hint}
            >
              {comparisonLine.label}
              {comparison?.left.frametime && comparison.right.frametime
                ? ` · Izq ${formatFrametimeLine(comparison.left.frametime)} · Der ${formatFrametimeLine(comparison.right.frametime)}`
                : ''}
            </p>
          )}
        </div>
      </div>
    </Panel>
  )
}
