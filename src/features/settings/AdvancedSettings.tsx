import { Input } from '../../shared/ui/Input'
import { useEffect, useState } from 'react'
import { ChevronDown } from 'lucide-react'
import { Button } from '../../shared/ui/Button'
import { DarkSelect } from '../../shared/ui/DarkSelect'
import { DataText } from '../../shared/ui/DataText'
import { open, save } from '@tauri-apps/plugin-dialog'
import { audioStatusLabel } from '../../shared/audio'
import { api } from '../../shared/api'
import { resolveRunner } from '../../shared/resolveRunner'
import { Panel } from '../../shared/ui/Panel'
import { StatusDot, type DotStatus } from '../../shared/ui/StatusDot'
import { useSelectedServer } from '../servers/useSelectedServer'
import { useLauncherStore } from '../launcher/launcher.store'
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
    <div
      className="min-w-0 border-t border-line first:border-t-0 pt-1"
      title={hint ?? undefined}
    >
      <div className="flex items-center gap-2 min-w-0">
        <StatusDot status={dotStatus} />
        <p className="text-detail font-sans text-muted min-w-0 break-words">
          <DataText>{label}</DataText>
        </p>
      </div>
      {hint && (
        <p
          className={`text-caption text-muted leading-snug pl-4 whitespace-normal break-words ${hint.includes('/') ? 'font-mono' : ''}`}
        >
          <DataText>{hint}</DataText>
        </p>
      )}
    </div>
  )
}

export function AdvancedSettings() {
  const [reviewOpen, setReviewOpen] = useState(false)
  const [observationCount, setObservationCount] = useState(0)
  const [benchmarkCount, setBenchmarkCount] = useState(0)
  const [benchmarkRuns, setBenchmarkRuns] = useState<BenchmarkRunSummary[]>([])
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
  const clients = useLauncherStore((state) => state.clients)
  const runningClientId =
    clients.find(
      (client) => client.status === 'running' && client.serverId === server?.id,
    )?.clientId ?? ''

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
    if (checked) {
      const oppositeSetter = side === 'left' ? setCompareRight : setCompareLeft
      oppositeSetter((prev) => {
        const next = new Set(prev)
        next.delete(runIdToToggle)
        return next
      })
    }
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
      size="compact"
      tone={hasIssue ? 'warn' : 'neutral'}
      className="shrink-0"
    >
      <div className="space-y-1">
        {lines
          .filter((line) => !['observations', 'benchmarks'].includes(line.key))
          .map((line) => (
            <StatusLine
              key={line.key}
              dotStatus={line.dot}
              label={line.label}
              hint={line.hint}
            />
          ))}
        <details
          open={reviewOpen}
          onToggle={(event) => setReviewOpen(event.currentTarget.open)}
          className="border-t border-line pt-2"
        >
          <summary
            title={
              lines.find((line) => line.key === 'benchmarks')?.hint ?? undefined
            }
            className="flex cursor-pointer list-none items-center justify-between gap-2 text-detail text-muted focus-visible:outline focus-visible:outline-accent [&::-webkit-details-marker]:hidden"
          >
            <span>
              <DataText>{benchmarksLabel(benchmarkCount)}</DataText>
            </span>
            <ChevronDown
              className={`h-3 w-3 shrink-0 ${reviewOpen ? 'rotate-180' : ''}`}
              aria-hidden
            />
          </summary>
          <div className="space-y-1 pt-2">
            {lines
              .filter((line) => line.key === 'observations')
              .map((line) => (
                <StatusLine
                  key={line.key}
                  dotStatus={line.dot}
                  label={line.label}
                  hint={line.hint}
                />
              ))}
            <div className="flex flex-wrap gap-2 pt-1 pl-4">
              <Button
                variant="ghost"
                size="xs"
                type="button"
                onClick={() => void exportObservations()}
              >
                Exportar observaciones
              </Button>
              <Button
                variant="ghost"
                size="xs"
                type="button"
                onClick={() => void deleteObservations()}
              >
                Borrar
              </Button>
            </div>
            <div className="mt-2 space-y-1.5 pl-4 border-t border-line pt-2">
              <p className="text-caption text-muted leading-snug">
                {lines.find((line) => line.key === 'benchmarks')?.hint}
              </p>
              <p className="text-caption text-muted">
                Cliente en ejecución:{' '}
                <span className="font-mono tabular-nums">
                  {runningClientId ? runningClientId.slice(0, 8) : 'ninguno'}
                </span>
                {activeRunId ? (
                  <span className="font-mono tabular-nums">{` · run ${activeRunId.slice(0, 8)}`}</span>
                ) : (
                  ''
                )}
              </p>
              <div className="flex flex-wrap gap-1.5 text-caption">
                <DarkSelect
                  role="combobox"
                  size="sm"
                  value={arm}
                  onChange={(value) => setArm(value as 'a' | 'b')}
                  options={[
                    { value: 'a', label: 'Brazo A' },
                    { value: 'b', label: 'Brazo B' },
                  ]}
                />
                <Input
                  variant="inline"
                  className="font-mono tabular-nums w-24 bg-field border border-line-strong rounded-inline px-1 py-0.5"
                  value={sceneId}
                  onChange={(e) => setSceneId(e.target.value)}
                  placeholder="scene"
                />
                <Input
                  variant="inline"
                  className="font-mono tabular-nums w-16 bg-field border border-line-strong rounded-inline px-1 py-0.5"
                  value={loadDescriptor}
                  onChange={(e) => setLoadDescriptor(e.target.value)}
                  placeholder="load"
                />
                <Input
                  variant="inline"
                  type="number"
                  className="font-mono w-14 bg-field border border-line-strong rounded-inline px-1 py-0.5"
                  value={width}
                  onChange={(e) => setWidth(Number(e.target.value))}
                />
                <Input
                  variant="inline"
                  type="number"
                  className="font-mono w-14 bg-field border border-line-strong rounded-inline px-1 py-0.5"
                  value={height}
                  onChange={(e) => setHeight(Number(e.target.value))}
                />
                <label className="flex items-center gap-1 text-muted">
                  <Input
                    variant="inline"
                    type="checkbox"
                    checked={fullscreen}
                    onChange={(e) => setFullscreen(e.target.checked)}
                  />
                  FS
                </label>
                <Input
                  variant="inline"
                  type="number"
                  className="font-mono w-12 bg-field border border-line-strong rounded-inline px-1 py-0.5"
                  value={warmupSeconds}
                  onChange={(e) => setWarmupSeconds(Number(e.target.value))}
                  title="warmup s"
                />
                <Input
                  variant="inline"
                  type="number"
                  className="font-mono w-12 bg-field border border-line-strong rounded-inline px-1 py-0.5"
                  value={captureSeconds}
                  onChange={(e) => setCaptureSeconds(Number(e.target.value))}
                  title="capture s"
                />
              </div>
              <div className="flex flex-wrap gap-2">
                <Button
                  variant="ghost"
                  size="xs"
                  type="button"
                  disabled={benchmarkBusy || !server || !runningClientId}
                  onClick={() => void attachBenchmark()}
                >
                  Adjuntar
                </Button>
                <Button
                  variant="ghost"
                  size="xs"
                  type="button"
                  disabled={benchmarkBusy || !runId}
                  onClick={() => void beginCapture()}
                >
                  Iniciar captura
                </Button>
                <Button
                  variant="ghost"
                  size="xs"
                  type="button"
                  disabled={benchmarkBusy || !runId}
                  onClick={() => void finishCapture()}
                >
                  Terminar captura
                </Button>
                <Button
                  variant="ghost"
                  size="xs"
                  type="button"
                  disabled={benchmarkBusy || !runId}
                  onClick={() => void importCsv()}
                >
                  Importar CSV
                </Button>
                <Button
                  variant="ghost"
                  size="xs"
                  type="button"
                  disabled={benchmarkBusy || !runId}
                  onClick={() => void setVisual('passed')}
                >
                  Visual OK
                </Button>
                <Button
                  variant="ghost"
                  size="xs"
                  type="button"
                  disabled={benchmarkBusy || !runId}
                  onClick={() => void setVisual('failed')}
                >
                  Visual fallo
                </Button>
                <Button
                  variant="ghost"
                  size="xs"
                  type="button"
                  disabled={benchmarkBusy || !runId}
                  onClick={() => void setVisual('skipped')}
                >
                  Visual omitir
                </Button>
              </div>
              {benchmarkRuns.length > 0 && (
                <div className="space-y-0.5 text-caption text-muted">
                  {benchmarkRuns.map((run) => (
                    <div
                      key={run.runId}
                      className="flex flex-wrap items-center gap-2 border-t border-line pt-1"
                    >
                      <label className="flex items-center gap-0.5">
                        <Input
                          variant="inline"
                          type="radio"
                          name="active-benchmark-run"
                          checked={activeRunId === run.runId}
                          onChange={() => setActiveRunId(run.runId)}
                        />
                        Usar
                      </label>
                      <label className="flex items-center gap-0.5">
                        <Input
                          variant="inline"
                          type="checkbox"
                          checked={compareLeft.has(run.runId)}
                          disabled={run.arm !== 'a'}
                          onChange={(e) =>
                            toggleCompare('left', run.runId, e.target.checked)
                          }
                        />
                        Izq
                      </label>
                      <label className="flex items-center gap-0.5">
                        <Input
                          variant="inline"
                          type="checkbox"
                          checked={compareRight.has(run.runId)}
                          disabled={run.arm !== 'b'}
                          onChange={(e) =>
                            toggleCompare('right', run.runId, e.target.checked)
                          }
                        />
                        Der
                      </label>
                      <span className="font-mono tabular-nums min-w-0 break-words">
                        {run.arm} · {run.sceneId} · {run.recordState}
                      </span>
                    </div>
                  ))}
                </div>
              )}
              <div className="flex flex-wrap gap-2">
                <Button
                  variant="ghost"
                  size="xs"
                  type="button"
                  disabled={
                    benchmarkBusy ||
                    compareLeft.size === 0 ||
                    compareRight.size === 0
                  }
                  onClick={() => void runCompare()}
                >
                  Comparar
                </Button>
                <Button
                  variant="ghost"
                  size="xs"
                  type="button"
                  onClick={() => void exportBenchmarks()}
                >
                  Exportar benchmarks
                </Button>
                <Button
                  variant="ghost"
                  size="xs"
                  type="button"
                  disabled={
                    benchmarkBusy ||
                    compareLeft.size === 0 ||
                    compareRight.size === 0
                  }
                  onClick={() => void exportComparison()}
                >
                  Exportar comparación
                </Button>
                <Button
                  variant="ghost"
                  size="xs"
                  type="button"
                  onClick={() => void deleteBenchmarks()}
                >
                  Borrar benchmarks
                </Button>
              </div>
              {comparisonLine && (
                <p
                  className="text-caption text-muted"
                  title={comparisonLine.hint}
                >
                  <DataText>
                    {comparisonLine.label +
                      (comparison?.left.frametime && comparison.right.frametime
                        ? ` · Izq ${formatFrametimeLine(comparison.left.frametime)} · Der ${formatFrametimeLine(comparison.right.frametime)}`
                        : '')}
                  </DataText>
                </p>
              )}
            </div>
          </div>
        </details>
      </div>
    </Panel>
  )
}
