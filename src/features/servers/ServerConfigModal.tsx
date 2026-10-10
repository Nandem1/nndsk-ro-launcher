import { open } from '@tauri-apps/plugin-dialog'
import { useEffect, useId, useState } from 'react'
import { X } from 'lucide-react'
import { useAsyncAction } from '../../shared/hooks/useAsyncAction'
import { basename, nameFromExePath } from '../../shared/path'
import { runnerSelectionOptions } from '../../shared/resolveRunner'
import type { ServerConfig } from '../../shared/types'
import { Button, IconButton } from '../../shared/ui/Button'
import { DarkSelect } from '../../shared/ui/DarkSelect'
import { Input } from '../../shared/ui/Input'
import { ModalShell, modalSurfaceClasses } from '../../shared/ui/ModalShell'
import { useSettingsStore } from '../settings/settings.store'
import {
  createServerConfigDraft,
  serverFieldsFromDraft,
  type ServerConfigDraft,
  type ServerConfigFields,
  validateServerConfigDraft,
} from './serverConfig.logic'

type ActionKey = 'pick-game' | 'pick-patcher' | 'save'

interface Props {
  mode: 'add' | 'edit'
  server?: ServerConfig
  onSave: (fields: ServerConfigFields) => Promise<void>
  onClose: () => void
}

const STRATEGY_OPTIONS = [
  { value: 'direct', label: 'Ejecutar el juego directamente' },
  { value: 'patcher', label: 'Iniciar mediante el patcher' },
]

function FieldLabel({ children }: { children: string }) {
  return (
    <span className="text-detail text-muted uppercase tracking-wider">
      {children}
    </span>
  )
}

function TextInput({
  value,
  onChange,
  placeholder,
  autoFocus,
  monospace = false,
}: {
  value: string
  onChange: (value: string) => void
  placeholder?: string
  autoFocus?: boolean
  monospace?: boolean
}) {
  return (
    <Input
      variant="config"
      autoFocus={autoFocus}
      type="text"
      value={value}
      onChange={(event) => onChange(event.target.value)}
      placeholder={placeholder}
      spellCheck={false}
      className={monospace ? 'font-mono text-label' : ''}
    />
  )
}

function ExecutablePicker({
  label,
  path,
  placeholder,
  busy,
  optional = false,
  onPick,
  onClear,
}: {
  label: string
  path: string
  placeholder: string
  busy: boolean
  optional?: boolean
  onPick: () => void
  onClear?: () => void
}) {
  return (
    <div className="flex flex-col gap-1.5 min-w-0">
      <div className="flex items-center justify-between gap-2">
        <FieldLabel>{label}</FieldLabel>
        {optional && <span className="text-caption text-muted">Opcional</span>}
      </div>
      <div className="flex gap-1.5 min-w-0">
        <button
          type="button"
          onClick={onPick}
          disabled={busy}
          className="min-w-0 flex-1 flex items-center justify-between gap-3 bg-field border border-line-strong
            rounded-control px-3 py-2.5 text-sm text-left hover:border-accent/40 transition-colors
            disabled:opacity-50 disabled:cursor-wait"
        >
          <span className={path ? 'font-mono text-ink truncate' : 'text-muted'}>
            {busy ? 'Abriendo...' : path ? basename(path) : placeholder}
          </span>
          <span className="text-xs text-accent shrink-0">Examinar</span>
        </button>
        {path && onClear && (
          <IconButton
            label={`Quitar ${label.toLowerCase()}`}
            variant="ghost"
            size="md"
            onClick={onClear}
          >
            <X className="w-3.5 h-3.5" />
          </IconButton>
        )}
      </div>
      {path && (
        <p
          className="text-caption text-muted font-mono truncate px-1"
          title={path}
        >
          {path}
        </p>
      )}
    </div>
  )
}

function ArgEditor({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string
  value: string
  onChange: (value: string) => void
  placeholder: string
}) {
  return (
    <label className="flex flex-col gap-1.5 min-w-0">
      <FieldLabel>{label}</FieldLabel>
      <textarea
        rows={3}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        spellCheck={false}
        className="resize-y bg-field border border-line-strong rounded-control px-3 py-2 text-detail leading-relaxed
          font-mono text-ink placeholder:text-muted focus:outline-none focus:border-accent/60 "
      />
    </label>
  )
}

export function ServerConfigModal({ mode, server, onSave, onClose }: Props) {
  const titleId = useId()
  const formId = useId()
  const [draft, setDraft] = useState<ServerConfigDraft>(() =>
    createServerConfigDraft(server),
  )
  const [validationError, setValidationError] = useState<string | null>(null)
  const { error, run, isBusy, busyKey } = useAsyncAction<ActionKey>()
  const runners = useSettingsStore((state) => state.runners)
  const selectedRunner = useSettingsStore((state) => state.selectedRunner)
  const runnerOptions = [
    {
      value: '',
      label: `Predeterminado${selectedRunner ? ` · ${basename(selectedRunner)}` : ''}`,
    },
    ...runnerSelectionOptions(runners, draft.runner),
  ]

  const title = mode === 'add' ? 'Agregar servidor' : 'Editar servidor'
  const saveLabel = mode === 'add' ? 'Agregar' : 'Guardar cambios'

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isBusy('save')) onClose()
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [isBusy, onClose])

  const setField = <K extends keyof ServerConfigDraft>(
    key: K,
    value: ServerConfigDraft[K],
  ) => {
    setDraft((current) => ({ ...current, [key]: value }))
    setValidationError(null)
  }

  const pickExecutable = async (kind: 'game' | 'patcher') => {
    const action: ActionKey = kind === 'game' ? 'pick-game' : 'pick-patcher'
    await run(action, async () => {
      const selected = await open({
        multiple: false,
        directory: false,
        title:
          kind === 'game'
            ? 'Seleccionar ejecutable del cliente'
            : 'Seleccionar patcher',
        filters: [{ name: 'Ejecutable', extensions: ['exe'] }],
      })
      if (!selected || Array.isArray(selected)) return

      setDraft((current) => ({
        ...current,
        ...(kind === 'game'
          ? {
              executablePath: selected,
              name: current.name.trim()
                ? current.name
                : nameFromExePath(selected),
            }
          : { patcherPath: selected }),
      }))
      setValidationError(null)
    })
  }

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault()
    const draftError = validateServerConfigDraft(draft)
    if (draftError) {
      setValidationError(draftError)
      return
    }

    const ok = await run('save', () => onSave(serverFieldsFromDraft(draft)))
    if (ok) onClose()
  }

  const shownError = validationError ?? error
  const busy = busyKey !== null

  return (
    <ModalShell
      layer="server"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !isBusy('save')) onClose()
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className={`${modalSurfaceClasses('glass')} w-[560px] max-w-full max-h-[92vh]`}
      >
        <div className="flex items-start justify-between gap-3 px-6 pt-5 pb-4 border-b border-line">
          <div>
            <h3 id={titleId} className="text-ink font-semibold text-lg">
              {title}
            </h3>
            <p className="text-xs text-muted mt-1">
              Cliente y contrato de lanzamiento.
            </p>
          </div>
          <IconButton
            label="Cerrar configuración"
            variant="ghost"
            size="sm"
            onClick={onClose}
            disabled={isBusy('save')}
          >
            <X className="w-4 h-4" />
          </IconButton>
        </div>

        <form
          id={formId}
          onSubmit={handleSubmit}
          className="min-h-0 overflow-y-auto px-6 py-4 flex flex-col gap-5"
        >
          <section className="flex flex-col gap-3">
            <h4 className="text-detail font-semibold text-muted uppercase tracking-wider">
              Cliente
            </h4>
            <label className="flex flex-col gap-1.5">
              <FieldLabel>Nombre</FieldLabel>
              <TextInput
                autoFocus
                value={draft.name}
                onChange={(value) => setField('name', value)}
                placeholder="Mi Servidor RO"
              />
            </label>
            <ExecutablePicker
              label="Ejecutable del juego"
              path={draft.executablePath}
              placeholder="Seleccionar .exe..."
              busy={isBusy('pick-game')}
              onPick={() => void pickExecutable('game')}
            />
            <ExecutablePicker
              label="Patcher"
              path={draft.patcherPath}
              placeholder="Seleccionar patcher..."
              busy={isBusy('pick-patcher')}
              optional
              onPick={() => void pickExecutable('patcher')}
              onClear={() => setField('patcherPath', '')}
            />
          </section>

          <section className="flex flex-col gap-3 border-t border-line pt-4">
            <h4 className="text-detail font-semibold text-muted uppercase tracking-wider">
              Entorno
            </h4>
            <div className="rounded-control border-t border-line bg-panel px-3 py-2.5">
              <p className="text-xs text-ink">
                Entorno Ragnarok administrado automáticamente
              </p>
              <p className="mt-1 text-caption leading-relaxed text-muted">
                Este servidor tendrá su propio entorno aislado. El launcher
                descargará el runtime predeterminado y preparará las
                dependencias del runner elegido al jugar por primera vez.
              </p>
            </div>
            <div className="flex flex-col gap-1.5">
              <FieldLabel>Runner de este servidor</FieldLabel>
              <DarkSelect
                value={draft.runner}
                options={runnerOptions}
                onChange={(value) => setField('runner', value)}
              />
              <p className="text-caption leading-relaxed text-muted">
                Al fijar uno, el launcher usa un prefix distinto para cada
                runner. El entorno anterior no se modifica.
              </p>
            </div>
          </section>

          <section className="flex flex-col gap-3 border-t border-line pt-4">
            <h4 className="text-detail font-semibold text-muted uppercase tracking-wider">
              Lanzamiento
            </h4>
            <div className="flex flex-col gap-1.5">
              <FieldLabel>Estrategia</FieldLabel>
              <DarkSelect
                value={draft.strategy}
                options={STRATEGY_OPTIONS}
                onChange={(value) =>
                  setField('strategy', value as ServerConfigDraft['strategy'])
                }
              />
              {draft.strategy === 'patcher' && !draft.patcherPath && (
                <span className="text-caption text-warn">
                  Esta estrategia requiere seleccionar un patcher.
                </span>
              )}
            </div>

            <div className="grid grid-cols-2 gap-3 max-sm:grid-cols-1">
              <ArgEditor
                label="Argumentos del juego"
                value={draft.gameArgs}
                onChange={(value) => setField('gameArgs', value)}
                placeholder={'-1rag1\n${username}'}
              />
              <ArgEditor
                label="Argumentos del patcher"
                value={draft.patcherArgs}
                onChange={(value) => setField('patcherArgs', value)}
                placeholder={'--server=sakura\n${username}'}
              />
            </div>
            <p className="text-caption text-muted leading-relaxed">
              Una línea equivale a un argumento completo. Para credenciales usa
              plantillas como{' '}
              <code className="text-muted">{'${username}'}</code>; no guardes
              valores secretos en esta configuración.
            </p>
            <label className="flex items-start gap-2 rounded-control border border-line bg-surface px-3 py-2.5">
              <input
                type="checkbox"
                checked={draft.requireWebview2}
                onChange={(event) =>
                  setField('requireWebview2', event.target.checked)
                }
                className="mt-0.5 accent-accent"
              />
              <span className="text-caption leading-relaxed text-muted">
                Forzar Microsoft Edge WebView2. Úsalo si el patcher tiene una
                interfaz web y el análisis PE aparece como inconcluso.
              </span>
            </label>
          </section>
        </form>

        <div className="px-6 py-4 border-t border-line bg-surface">
          {shownError && (
            <p role="alert" className="text-xs text-bad mb-3">
              {shownError}
            </p>
          )}
          <div className="flex gap-2">
            <Button
              variant="secondary"
              size="md"
              block
              onClick={onClose}
              disabled={isBusy('save')}
            >
              Cancelar
            </Button>
            <Button
              type="submit"
              form={formId}
              variant="primary"
              size="md"
              block
              disabled={busy}
            >
              {isBusy('save') ? 'Guardando...' : saveLabel}
            </Button>
          </div>
        </div>
      </div>
    </ModalShell>
  )
}
