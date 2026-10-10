import { Input } from '../../shared/ui/Input'
import { useEffect, useRef, useState } from 'react'
import { Search } from 'lucide-react'
import { DEFAULT_AUTOPOT_CONFIG, POT_KEYS } from '../../shared/constants'

const POT_KEY_OPTIONS = POT_KEYS.map((key) => ({ value: key, label: key }))
const DETECTED_PROFILE_VALUE = '__detected_memory__'
import { Panel, resolveToolTone } from '../../shared/ui/Panel'
import { DarkSelect } from '../../shared/ui/DarkSelect'
import { ToggleSwitch } from '../../shared/ui/ToggleSwitch'
import { useSelectedServer } from '../servers/useSelectedServer'
import { useLauncherStore } from '../launcher/launcher.store'
import { useUiModeStore } from '../../app/uiMode.store'
import { statPercent } from './autopot.logic'
import { useAutopot } from './useAutopot'
import {
  memoryAccessAction,
  memoryAccessLabel,
  memoryAccessUsable,
} from './memoryAccess.logic'
import { api } from '../../shared/api'
import type { ClientProfile } from '../../shared/types'
import { MemoryScannerModal } from './MemoryScannerModal'

function StatBar({
  cur,
  max,
  tone,
  flash,
}: {
  cur: number
  max: number
  tone: 'bad' | 'info'
  flash?: boolean
}) {
  const empty = max <= 0
  const pct = statPercent(cur, max)
  const fillClass = tone === 'bad' ? 'bg-bad' : 'bg-info'

  const flashClass = flash ? 'animate-pulse-dot' : ''

  return (
    <div className={`space-y-1 pt-2 ${flashClass}`}>
      <div className="flex justify-between text-caption text-muted">
        <span className="micro-label">{tone === 'bad' ? 'HP' : 'SP'}</span>
        <span className="font-mono">
          {empty
            ? '— / —'
            : `${cur.toLocaleString()} / ${max.toLocaleString()} (${pct}%)`}
        </span>
      </div>
      <div className="h-1.5 rounded-pill overflow-hidden bg-panel-raised">
        {!empty && (
          <div
            className={`h-full rounded-pill ${fillClass} transition-colors duration-120`}
            style={{ width: `${pct}%` }}
          />
        )}
      </div>
    </div>
  )
}

export function AutopotPanel() {
  const server = useSelectedServer()
  const { config, status, busy, isRunning, error, setEnabled, updateField } =
    useAutopot(server)
  const launching = useLauncherStore((s) => s.status === 'launching')
  const multipleClients = useLauncherStore((s) => s.clients.length > 1)
  const clientMemoryAccess = useLauncherStore((s) =>
    s.clients.length === 1 ? s.clients[0].memoryAccess : null,
  )
  const hero = useUiModeStore((s) => s.mode === 'ingame')
  const available = isRunning && !!server
  const minimumDelayMs = 10
  const hasCharacter = available && !!status.characterName
  const [flashHp, setFlashHp] = useState(false)
  const [flashSp, setFlashSp] = useState(false)
  const prevHp = useRef(0)
  const prevSp = useRef(0)
  const [profiles, setProfiles] = useState<ClientProfile[]>([])
  const [showMemoryScanner, setShowMemoryScanner] = useState(false)
  const hasMemoryOverride =
    !!config.hpBaseOverride ||
    !!config.nameAddressOverride ||
    !!config.levelAddressOverride ||
    !!config.mapAddressOverride

  useEffect(() => {
    void api.listClientProfiles().then(setProfiles).catch(console.error)
  }, [])

  const effectiveMemoryAccess = status.memoryAccess ?? clientMemoryAccess
  const memoryReady = memoryAccessUsable(effectiveMemoryAccess)
  const memoryAction = memoryAccessAction(effectiveMemoryAccess)
  const showProfileHint =
    available &&
    config.enabled &&
    (status.profileMemory === 'addressUnmapped' ||
      status.profileMemory === 'invalidRead')

  useEffect(() => {
    if (!available || status.maxHp <= 0) {
      prevHp.current = 0
      return
    }
    if (status.curHp > prevHp.current && prevHp.current > 0) {
      setFlashHp(true)
      const t = setTimeout(() => setFlashHp(false), 450)
      prevHp.current = status.curHp
      return () => clearTimeout(t)
    }
    prevHp.current = status.curHp
  }, [available, status.curHp, status.maxHp])

  useEffect(() => {
    if (!available || status.maxSp <= 0) {
      prevSp.current = 0
      return
    }
    if (status.curSp > prevSp.current && prevSp.current > 0) {
      setFlashSp(true)
      const t = setTimeout(() => setFlashSp(false), 450)
      prevSp.current = status.curSp
      return () => clearTimeout(t)
    }
    prevSp.current = status.curSp
  }, [available, status.curSp, status.maxSp])

  const displayName = available
    ? status.characterName || server!.name
    : (server?.name ?? 'Sin servidor')

  const statusText = !server
    ? 'Selecciona un servidor'
    : launching
      ? 'Iniciando juego...'
      : multipleClients
        ? 'No disponible con varios clientes'
        : !isRunning
          ? 'Inicia el juego'
          : status.active
            ? 'Activo'
            : 'Inactivo'

  const hpCur =
    available && (status.active || config.enabled) ? status.curHp : 0
  const hpMax =
    available && (status.active || config.enabled) ? status.maxHp : 0
  const spCur =
    available && (status.active || config.enabled) ? status.curSp : 0
  const spMax =
    available && (status.active || config.enabled) ? status.maxSp : 0

  const tone = resolveToolTone(
    available && memoryReady,
    config.enabled &&
      status.active &&
      memoryAccessUsable(effectiveMemoryAccess),
    !!error || (available && !memoryReady),
  )

  return (
    <Panel
      title="AutoPot"
      size={hero ? 'hero' : 'compact'}
      tone={tone}
      className="h-full"
    >
      <div
        data-design-panel-scroll
        className={`flex-1 min-h-0 ${hero ? 'overflow-y-auto space-y-2 pr-0.5' : 'combat-fit space-y-1'}`}
      >
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <p
              className={`truncate text-sm font-semibold ${hasCharacter ? 'text-ok' : 'text-ink'}`}
            >
              {displayName}
            </p>
            <p
              className={`text-caption ${launching ? 'text-muted animate-pulse-dot' : 'text-muted'}`}
            >
              {statusText}
            </p>
          </div>
          <ToggleSwitch
            checked={config.enabled && available && memoryReady}
            disabled={!available || !memoryReady || busy}
            onChange={(enabled) => void setEnabled(enabled)}
            tone="ok"
          />
        </div>

        <StatBar cur={hpCur} max={hpMax} tone="bad" flash={flashHp} />
        <StatBar cur={spCur} max={spMax} tone="info" flash={flashSp} />

        <div className="flex items-center justify-between gap-2 py-2">
          <div className="min-w-0">
            <p className="text-detail font-medium text-ink">Modo proactivo</p>
            <p className="text-caption leading-snug text-muted">
              Envía HP entre recuperaciones para reducir la reacción con
              latencia alta.
            </p>
          </div>
          <ToggleSwitch
            checked={config.proactiveMode}
            disabled={!server || busy}
            onChange={(proactiveMode) => void updateField({ proactiveMode })}
            tone="ok"
          />
        </div>

        <div className="flex items-center gap-2 min-h-6 pt-2">
          <span className="micro-label shrink-0">Lectura</span>
          <Input
            variant="inline"
            type="range"
            min={minimumDelayMs}
            max={200}
            step={1}
            disabled={!server || busy}
            value={Math.max(config.delayMs, minimumDelayMs)}
            onChange={(event) =>
              void updateField({ delayMs: Number(event.target.value) })
            }
            className="font-mono flex-1 idle-control"
          />
          <span className="text-caption font-mono text-muted w-10 text-right shrink-0">
            {status.active
              ? status.effectiveDelayMs
              : Math.max(config.delayMs, minimumDelayMs)}
            ms
          </span>
        </div>

        <div className="space-y-1 pt-2">
          <div className="flex items-center justify-between gap-2">
            <span className="micro-label">Perfil de memoria</span>
            <button
              type="button"
              disabled={!available || status.active || busy}
              onClick={() => setShowMemoryScanner(true)}
              className="idle-control inline-flex items-center gap-1 text-caption text-muted hover:text-ink focus-visible:outline focus-visible:outline-accent disabled:text-muted"
            >
              <Search className="h-3 w-3" aria-hidden />
              Encontrar
            </button>
          </div>
          <DarkSelect
            size="sm"
            value={
              hasMemoryOverride
                ? DETECTED_PROFILE_VALUE
                : (config.profileId ?? '')
            }
            disabled={!server}
            onChange={(val) => {
              if (val === DETECTED_PROFILE_VALUE) return
              void updateField({
                profileId: val || undefined,
                hpBaseOverride: undefined,
                nameAddressOverride: undefined,
                levelAddressOverride: undefined,
                jobLevelAddressOverride: undefined,
                mapAddressOverride: undefined,
              })
            }}
            options={[
              ...(hasMemoryOverride
                ? [
                    {
                      value: DETECTED_PROFILE_VALUE,
                      label: `Detectado · ${config.hpBaseOverride ?? 'nombre'}`,
                    },
                  ]
                : []),
              { value: '', label: 'Auto' },
              ...profiles.map((p) => ({ value: p.id, label: p.label })),
            ]}
          />
        </div>

        <div className="grid grid-cols-2 gap-2 pt-2">
          <div className="space-y-1">
            <span className="micro-label">HP</span>
            <div className="flex gap-1">
              <DarkSelect
                size="sm"
                variant="keycap"
                value={config.hpKey}
                disabled={!server}
                onChange={(hpKey) => void updateField({ hpKey })}
                options={POT_KEY_OPTIONS}
              />
              <div className="relative w-12 shrink-0">
                <Input
                  variant="inline"
                  type="number"
                  min={1}
                  max={99}
                  inputMode="numeric"
                  aria-label="Porcentaje de HP"
                  disabled={!server}
                  value={config.hpPercent}
                  onChange={(e) =>
                    void updateField({
                      hpPercent:
                        Number(e.target.value) ||
                        DEFAULT_AUTOPOT_CONFIG.hpPercent,
                    })
                  }
                  className="font-mono input-no-spinner w-full rounded-control border border-line bg-field py-1 pl-1.5 pr-4 text-center text-data text-ink outline-none transition-colors hover:border-line-strong idle-control"
                />
                <span className="font-mono tabular-nums pointer-events-none absolute inset-y-0 right-1.5 flex items-center text-micro text-muted">
                  %
                </span>
              </div>
            </div>
          </div>
          <div className="space-y-1">
            <span className="micro-label">SP</span>
            <div className="flex gap-1">
              <DarkSelect
                size="sm"
                variant="keycap"
                value={config.spKey}
                disabled={!server}
                onChange={(spKey) => void updateField({ spKey })}
                options={POT_KEY_OPTIONS}
              />
              <div className="relative w-12 shrink-0">
                <Input
                  variant="inline"
                  type="number"
                  min={1}
                  max={99}
                  inputMode="numeric"
                  aria-label="Porcentaje de SP"
                  disabled={!server}
                  value={config.spPercent}
                  onChange={(e) =>
                    void updateField({
                      spPercent:
                        Number(e.target.value) ||
                        DEFAULT_AUTOPOT_CONFIG.spPercent,
                    })
                  }
                  className="font-mono input-no-spinner w-full rounded-control border border-line bg-field py-1 pl-1.5 pr-4 text-center text-data text-ink outline-none transition-colors hover:border-line-strong idle-control"
                />
                <span className="font-mono tabular-nums pointer-events-none absolute inset-y-0 right-1.5 flex items-center text-micro text-muted">
                  %
                </span>
              </div>
            </div>
          </div>
        </div>

        <p className="text-caption leading-snug min-h-[calc(1em*1.375)]">
          {error && available ? (
            <span className="text-bad">{error}</span>
          ) : available && effectiveMemoryAccess && !memoryReady ? (
            <span className="inline-block notice-warn text-muted">
              {memoryAccessLabel(effectiveMemoryAccess)}
              <span className="text-muted">
                {memoryAction ? ` ${memoryAction}` : ''}
              </span>
            </span>
          ) : showProfileHint ? (
            <span className="text-muted">
              La dirección del perfil no es válida; usa Encontrar o revisa el
              perfil.
            </span>
          ) : null}
        </p>
      </div>
      {showMemoryScanner && server && (
        <MemoryScannerModal
          serverName={server.name}
          existingHpBase={config.hpBaseOverride}
          onCancel={() => setShowMemoryScanner(false)}
          onConfirm={async (result) => {
            await updateField({
              hpBaseOverride: result.hpBase,
              nameAddressOverride: result.nameAddress,
              levelAddressOverride: result.levelAddress,
              jobLevelAddressOverride: result.jobLevelAddress,
              mapAddressOverride: result.mapAddress,
              profileId: undefined,
            })
          }}
        />
      )}
    </Panel>
  )
}
