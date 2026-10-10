import { useEffect, useId, useState } from 'react'
import { createPortal } from 'react-dom'
import { api } from '../../shared/api'
import { runSafely } from '../../shared/async'
import { Button } from '../../shared/ui/Button'
import { Input } from '../../shared/ui/Input'
import { ModalShell, modalSurfaceClasses } from '../../shared/ui/ModalShell'
import type {
  DetectedLevelAddress,
  DetectedMapAddress,
  DetectedNameAddress,
  DetectedMemoryLayout,
  LevelScanProgress,
  MapScanProgress,
  MemoryScanProgress,
  MemoryScanResult,
} from '../../shared/types'

interface Props {
  serverName: string
  existingHpBase?: string
  onCancel: () => void
  onConfirm: (result: MemoryScanResult) => Promise<void>
}

type ScanStep =
  | 'initial'
  | 'refine'
  | 'name'
  | 'discord'
  | 'level'
  | 'levelRefine'
  | 'map'
  | 'mapRefine'
  | 'confirmed'

export function parseHp(value: string): number | null {
  if (!/^\d+$/.test(value.trim())) return null
  const parsed = Number(value)
  return Number.isSafeInteger(parsed) && parsed > 0 && parsed <= 0xffffffff
    ? parsed
    : null
}

export function parseLevel(value: string): number | null {
  if (!/^\d+$/.test(value.trim())) return null
  const parsed = Number(value)
  return Number.isSafeInteger(parsed) && parsed >= 1 && parsed <= 300
    ? parsed
    : null
}

export function parseMapName(value: string): string | null {
  const trimmed = value.trim().toLowerCase()
  const stripped = trimmed.replace(/\.(rsw|gat)$/i, '')
  if (!stripped || stripped.length > 39) return null
  if (!/^[a-z0-9_@-]+$/.test(stripped)) return null
  return stripped
}

export function MemoryScannerModal({
  serverName,
  existingHpBase,
  onCancel,
  onConfirm,
}: Props) {
  const titleId = useId()
  const [step, setStep] = useState<ScanStep>(
    existingHpBase ? 'name' : 'initial',
  )
  const [hp, setHp] = useState('')
  const [progress, setProgress] = useState<MemoryScanProgress | null>(null)
  const [levelProgress, setLevelProgress] = useState<LevelScanProgress | null>(
    null,
  )
  const [mapProgress, setMapProgress] = useState<MapScanProgress | null>(null)
  const [confirmed, setConfirmed] = useState<DetectedMemoryLayout | null>(null)
  const [name, setName] = useState('')
  const [detectedName, setDetectedName] = useState<DetectedNameAddress | null>(
    null,
  )
  const [level, setLevel] = useState('')
  const [detectedLevel, setDetectedLevel] =
    useState<DetectedLevelAddress | null>(null)
  const [mapName, setMapName] = useState('')
  const [detectedMap, setDetectedMap] = useState<DetectedMapAddress | null>(
    null,
  )
  const [lastHp, setLastHp] = useState<number | null>(null)
  const [lastLevel, setLastLevel] = useState<number | null>(null)
  const [lastMap, setLastMap] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const parsedHp = parseHp(hp)
  const parsedLevel = parseLevel(level)
  const parsedMap = parseMapName(mapName)
  const resolvedHpBase = confirmed?.hpBase ?? existingHpBase

  const cancel = () => {
    void api.cancelAutopotMemoryScan()
    onCancel()
  }

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        void api.cancelAutopotMemoryScan()
        onCancel()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [onCancel])

  const skipPresence = () => {
    void api.cancelAutopotMemoryScan().catch(() => undefined)
    setDetectedLevel(null)
    setDetectedMap(null)
    setLevel('')
    setMapName('')
    setLevelProgress(null)
    setMapProgress(null)
    setLastLevel(null)
    setLastMap(null)
    setError(null)
    setStep('confirmed')
  }

  const submitHp = async (event: React.FormEvent) => {
    event.preventDefault()
    if (parsedHp === null || busy) return
    if (step === 'refine' && parsedHp === lastHp) {
      setError('El HP no cambió. Pierde o recupera HP antes de continuar')
      return
    }
    setBusy(true)
    setError(null)
    const result = await runSafely(() =>
      step === 'initial'
        ? api.beginAutopotMemoryScan(parsedHp)
        : api.refineAutopotMemoryScan(parsedHp),
    )
    setBusy(false)
    if (!result.ok) {
      setError(result.error)
      if (step === 'refine') {
        void api.cancelAutopotMemoryScan()
        setStep('initial')
        setProgress(null)
        setLastHp(null)
      }
      return
    }

    setProgress(result.value)
    setLastHp(parsedHp)
    if (result.value.confirmed) {
      setConfirmed(result.value.confirmed)
      setStep('name')
      setHp('')
    } else {
      setStep('refine')
      setHp('')
    }
  }

  const submitName = async (event: React.FormEvent) => {
    event.preventDefault()
    const characterName = name.trim()
    if (!characterName || busy) return
    setBusy(true)
    setError(null)
    const result = await runSafely(() =>
      api.findAutopotNameAddress(characterName, resolvedHpBase),
    )
    setBusy(false)
    if (!result.ok) {
      setError(result.error)
      return
    }
    setDetectedName(result.value)
    setStep('discord')
  }

  const submitLevel = async (event: React.FormEvent) => {
    event.preventDefault()
    if (parsedLevel === null || busy) return
    if (step === 'levelRefine' && parsedLevel === lastLevel) {
      setError('El nivel no cambió. Sube de nivel antes de continuar')
      return
    }
    setBusy(true)
    setError(null)
    const result = await runSafely(() =>
      step === 'level'
        ? api.beginAutopotLevelScan(
            parsedLevel,
            detectedName?.nameAddress,
            resolvedHpBase,
          )
        : api.refineAutopotLevelScan(parsedLevel),
    )
    setBusy(false)
    if (!result.ok) {
      setError(result.error)
      if (step === 'levelRefine') {
        void api.cancelAutopotMemoryScan()
        setStep('level')
        setLevelProgress(null)
        setLastLevel(null)
      }
      return
    }

    setLevelProgress(result.value)
    setLastLevel(parsedLevel)
    if (result.value.confirmed) {
      setDetectedLevel(result.value.confirmed)
      setStep('map')
      setLevel('')
    } else {
      setStep('levelRefine')
      setLevel('')
    }
  }

  const submitMap = async (event: React.FormEvent) => {
    event.preventDefault()
    if (!parsedMap || busy) return
    if (step === 'mapRefine' && parsedMap === lastMap) {
      setError('El mapa no cambió. Cambia de mapa antes de continuar')
      return
    }
    setBusy(true)
    setError(null)
    const result = await runSafely(() =>
      step === 'map'
        ? api.beginAutopotMapScan(
            parsedMap,
            detectedName?.nameAddress,
            resolvedHpBase,
            detectedLevel?.levelAddress,
          )
        : api.refineAutopotMapScan(parsedMap),
    )
    setBusy(false)
    if (!result.ok) {
      setError(result.error)
      if (step === 'mapRefine') {
        void api.cancelAutopotMemoryScan()
        setStep('map')
        setMapProgress(null)
        setLastMap(null)
      }
      return
    }

    setMapProgress(result.value)
    setLastMap(parsedMap)
    if (result.value.confirmed) {
      setDetectedMap(result.value.confirmed)
      setStep('confirmed')
      setMapName('')
    } else {
      setStep('mapRefine')
      setMapName('')
    }
  }

  const save = async () => {
    if (!resolvedHpBase || busy) return
    setBusy(true)
    setError(null)
    const result = await runSafely(() =>
      onConfirm({
        hpBase: resolvedHpBase,
        nameAddress: detectedName?.nameAddress,
        ...(detectedLevel
          ? {
              levelAddress: detectedLevel.levelAddress,
              jobLevelAddress: detectedLevel.jobLevelAddress ?? undefined,
            }
          : {}),
        ...(detectedMap ? { mapAddress: detectedMap.mapAddress } : {}),
      }),
    )
    setBusy(false)
    if (!result.ok) {
      setError(result.error)
      return
    }
    onCancel()
  }

  const onSubmit =
    step === 'name'
      ? submitName
      : step === 'level' || step === 'levelRefine'
        ? submitLevel
        : step === 'map' || step === 'mapRefine'
          ? submitMap
          : step === 'initial' || step === 'refine'
            ? submitHp
            : (event: React.FormEvent) => event.preventDefault()

  const description =
    step === 'initial'
      ? 'Escribe el HP exacto que muestra el juego. Se buscará sólo en la memoria escribible del cliente.'
      : step === 'refine'
        ? `${progress?.candidateCount.toLocaleString() ?? 0} candidatos. Pierde o recupera HP y escribe el nuevo valor para releer las mismas direcciones.`
        : step === 'name'
          ? existingHpBase && !confirmed
            ? `HP base ${existingHpBase} ya configurado. Escribe el nombre exacto; se elige la copia más cercana al HP.`
            : 'HP/SP confirmado. Escribe el nombre exacto del personaje. Se elige la copia más cercana al HP, no la primera de memoria.'
          : step === 'discord'
            ? 'Opcional y sólo para Discord. AutoPot no necesita nivel ni mapa.'
            : step === 'level'
              ? 'Escribe el nivel base que ves ahora. Se buscará cerca de HP y nombre para evitar valores fijos.'
              : step === 'levelRefine'
                ? `${levelProgress?.candidateCount.toLocaleString() ?? 0} candidatos. Sube de nivel y escribe el nuevo valor.`
                : step === 'map'
                  ? 'Escribe el mapa actual (ej. prontera). Después cambia de mapa una vez para comparar las mismas direcciones.'
                  : step === 'mapRefine'
                    ? `${mapProgress?.candidateCount.toLocaleString() ?? 0} candidatos. Cambia de mapa una vez y escribe el nuevo nombre.`
                    : 'Revisa las direcciones antes de guardarlas para este servidor.'

  const busyLabel =
    step === 'initial'
      ? 'Escaneando memoria escribible…'
      : step === 'refine'
        ? 'Comparando candidatos…'
        : step === 'name'
          ? 'Buscando el nombre exacto…'
          : step === 'level' || step === 'levelRefine'
            ? 'Buscando el nivel…'
            : step === 'map' || step === 'mapRefine'
              ? 'Buscando el mapa…'
              : 'Guardando direcciones…'

  return createPortal(
    <ModalShell
      layer="scanner"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) cancel()
      }}
    >
      <form
        onSubmit={onSubmit}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className={`w-[410px] ${modalSurfaceClasses()}`}
      >
        <h3 id={titleId} className="text-base font-semibold text-ink">
          Encontrar memoria de {serverName}
        </h3>
        <p className="mt-1 text-xs leading-relaxed text-muted">{description}</p>

        {step === 'initial' || step === 'refine' ? (
          <label className="mt-4 flex flex-col gap-1.5">
            <span className="text-detail uppercase tracking-wider text-muted">
              {step === 'initial' ? 'HP actual' : 'Nuevo HP actual'}
            </span>
            <Input
              autoFocus
              type="number"
              min={1}
              max={0xffffffff}
              inputMode="numeric"
              value={hp}
              disabled={busy}
              onChange={(event) => setHp(event.target.value)}
              placeholder={step === 'initial' ? 'Ej. 13619' : 'Ej. 13430'}
              className="font-mono input-no-spinner disabled:opacity-50"
            />
          </label>
        ) : step === 'name' ? (
          <label className="mt-4 flex flex-col gap-1.5">
            <span className="text-detail uppercase tracking-wider text-muted">
              Nombre exacto
            </span>
            <Input
              autoFocus
              type="text"
              maxLength={39}
              value={name}
              disabled={busy}
              onChange={(event) => setName(event.target.value)}
              placeholder="Ej. NombrePJ"
              spellCheck={false}
              className="disabled:opacity-50"
            />
          </label>
        ) : step === 'level' || step === 'levelRefine' ? (
          <label className="mt-4 flex flex-col gap-1.5">
            <span className="text-detail uppercase tracking-wider text-muted">
              {step === 'level' ? 'Nivel actual' : 'Nuevo nivel'}
            </span>
            <Input
              autoFocus
              type="number"
              min={1}
              max={300}
              inputMode="numeric"
              value={level}
              disabled={busy}
              onChange={(event) => setLevel(event.target.value)}
              placeholder={step === 'level' ? 'Ej. 99' : 'Ej. 100'}
              className="font-mono input-no-spinner disabled:opacity-50"
            />
          </label>
        ) : step === 'map' || step === 'mapRefine' ? (
          <label className="mt-4 flex flex-col gap-1.5">
            <span className="text-detail uppercase tracking-wider text-muted">
              {step === 'map' ? 'Mapa actual' : 'Nuevo mapa'}
            </span>
            <Input
              autoFocus
              type="text"
              maxLength={39}
              value={mapName}
              disabled={busy}
              onChange={(event) => setMapName(event.target.value)}
              placeholder={step === 'map' ? 'Ej. prontera' : 'Ej. izlude'}
              spellCheck={false}
              className="disabled:opacity-50"
            />
          </label>
        ) : step === 'confirmed' && resolvedHpBase ? (
          <div className="mt-4 space-y-2 rounded-panel border border-ok/20 bg-panel p-3">
            <div className="flex items-center justify-between gap-3">
              <span className="text-detail text-muted">HP base</span>
              <code className="text-xs text-ok">{resolvedHpBase}</code>
            </div>
            <div className="flex items-center justify-between gap-3">
              <span className="text-detail text-muted">
                Nombre {detectedName ? `'${detectedName.characterName}'` : ''}
              </span>
              <code className="text-xs text-ink">
                {detectedName?.nameAddress ?? 'No configurado'}
              </code>
            </div>
            {detectedLevel && (
              <div className="flex items-center justify-between gap-3">
                <span className="text-detail text-muted">Nivel Discord</span>
                <code className="text-xs text-ink">
                  {detectedLevel.levelAddress}
                </code>
              </div>
            )}
            {detectedMap && (
              <div className="flex items-center justify-between gap-3">
                <span className="text-detail text-muted">
                  Mapa '{detectedMap.mapName}'
                </span>
                <code className="text-xs text-ink">
                  {detectedMap.mapAddress}
                </code>
              </div>
            )}
            {confirmed && (
              <>
                <div className="flex items-center justify-between gap-3 text-detail">
                  <span className="text-muted">Valores confirmados</span>
                  <span className="font-mono text-ink">
                    HP {confirmed.currentHp.toLocaleString()} /{' '}
                    {confirmed.maxHp.toLocaleString()} · SP{' '}
                    {confirmed.currentSp.toLocaleString()} /{' '}
                    {confirmed.maxSp.toLocaleString()}
                  </span>
                </div>
                <div className="flex items-center justify-between gap-3">
                  <span className="text-detail text-muted">
                    Buffer de estados
                  </span>
                  <code className="text-xs text-ink">
                    {confirmed.statusBuffer}
                  </code>
                </div>
              </>
            )}
          </div>
        ) : null}

        {busy && (
          <p className="mt-3 text-detail text-warn animate-pulse-dot">
            {busyLabel}
          </p>
        )}
        {error && <p className="mt-3 text-detail text-bad">{error}</p>}

        <div className="mt-5 flex gap-2">
          <Button
            type="button"
            onClick={cancel}
            variant="outline"
            size="dialog"
          >
            Cancelar
          </Button>
          {step === 'confirmed' ? (
            <Button
              type="button"
              disabled={busy || !resolvedHpBase}
              onClick={() => void save()}
              variant="solid"
              size="dialog"
              tone="ok"
            >
              Guardar direcciones
            </Button>
          ) : step === 'name' ? (
            <>
              <Button
                type="button"
                disabled={busy}
                onClick={() => {
                  if (existingHpBase && !confirmed) {
                    setStep('initial')
                    setError(null)
                    return
                  }
                  setStep('confirmed')
                }}
                variant="outline"
                size="dialog-sm"
                className="disabled:opacity-40"
              >
                {existingHpBase && !confirmed
                  ? 'Recalibrar HP'
                  : 'Omitir nombre'}
              </Button>
              <Button
                type="submit"
                disabled={busy || !name.trim()}
                variant="solid"
                size="dialog-sm"
              >
                Buscar nombre
              </Button>
            </>
          ) : step === 'discord' ? (
            <>
              <Button
                type="button"
                disabled={busy}
                onClick={skipPresence}
                variant="outline"
                size="dialog-sm"
                className="disabled:opacity-40"
              >
                Saltar
              </Button>
              <Button
                type="button"
                disabled={busy}
                onClick={() => {
                  setError(null)
                  setStep('level')
                }}
                variant="solid"
                size="dialog-sm"
              >
                Ubicar Discord
              </Button>
            </>
          ) : step === 'level' ||
            step === 'levelRefine' ||
            step === 'map' ||
            step === 'mapRefine' ? (
            <>
              <Button
                type="button"
                disabled={busy}
                onClick={skipPresence}
                variant="outline"
                size="dialog-sm"
                className="disabled:opacity-40"
              >
                Saltar
              </Button>
              <Button
                type="submit"
                disabled={
                  busy ||
                  (step === 'level' || step === 'levelRefine'
                    ? parsedLevel === null
                    : !parsedMap)
                }
                variant="solid"
                size="dialog-sm"
              >
                {step === 'level' || step === 'map' ? 'Buscar' : 'Comparar'}
              </Button>
            </>
          ) : (
            <Button
              type="submit"
              disabled={busy || parsedHp === null}
              variant="solid"
              size="dialog"
            >
              {step === 'initial' ? 'Buscar' : 'Comparar'}
            </Button>
          )}
        </div>
      </form>
    </ModalShell>,
    document.body,
  )
}
