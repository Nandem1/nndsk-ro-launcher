import { useEffect, useId } from 'react'
import { ChevronDown, X } from 'lucide-react'
import { basename } from '../../shared/path'
import { Button, IconButton } from '../../shared/ui/Button'
import { Input } from '../../shared/ui/Input'
import { DarkSelect } from '../../shared/ui/DarkSelect'
import { Checkbox } from '../../shared/ui/Checkbox'
import { ModalShell, modalSurfaceClasses } from '../../shared/ui/ModalShell'
import {
  INSTALLATION_LABELS,
  installationCanRestart,
  installationIsActive,
} from './installations.logic'
import { useInstallationWizard } from './useInstallationWizard'

interface Props {
  installationId?: string
  onClose: () => void
}

export function InstallationWizard({ installationId, onClose }: Props) {
  const titleId = useId()
  const {
    draft,
    runners,
    name,
    setName,
    installerPath,
    destinationPath,
    setDestinationPath,
    runner,
    setRunner,
    requireWebview2,
    setRequireWebview2,
    executablePath,
    setExecutablePath,
    patcherPath,
    setPatcherPath,
    candidates,
    hasScanned,
    pick,
    begin,
    scan,
    cancel,
    finish,
    busyKey,
    busy,
    error,
  } = useInstallationWizard(installationId, onClose)

  useEffect(() => {
    const handle = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !busy) onClose()
    }
    window.addEventListener('keydown', handle)
    return () => window.removeEventListener('keydown', handle)
  }, [busy, onClose])

  return (
    <ModalShell layer="server" role="presentation">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className={`${modalSurfaceClasses('glass')} w-[560px] max-w-full max-h-[92vh]`}
      >
        <div className="flex items-start justify-between gap-3 px-6 pt-5 pb-4 border-b border-line">
          <div>
            <h3 id={titleId} className="text-ink font-semibold text-lg">
              Instalar servidor
            </h3>
            <p className="text-xs text-muted mt-1">
              Completa el instalador y confirma el cliente resultante.
            </p>
          </div>
          <IconButton
            label="Cerrar instalación"
            variant="ghost"
            size="sm"
            onClick={onClose}
            disabled={busy}
          >
            <X className="w-4 h-4" />
          </IconButton>
        </div>
        <div
          data-design-panel-scroll
          className="min-h-0 overflow-y-auto px-6 py-4 flex flex-col gap-4"
        >
          {!draft && (
            <>
              <label className="flex flex-col gap-1.5 text-detail text-muted">
                Nombre del servidor
                <Input
                  variant="config"
                  autoFocus
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  disabled={busy}
                />
              </label>
              <Button disabled={busy} onClick={() => void pick('installer')}>
                {installerPath
                  ? basename(installerPath)
                  : 'Seleccionar instalador .exe'}
              </Button>
              {installerPath && (
                <p className="text-caption font-mono text-muted break-all">
                  {installerPath}
                </p>
              )}
              <label className="flex flex-col gap-1.5 text-detail text-muted">
                Runner para este servidor
                <DarkSelect
                  value={runner}
                  options={runners.map((item) => ({
                    value: item.path,
                    label: item.name,
                  }))}
                  onChange={setRunner}
                  disabled={busy}
                />
              </label>
              <p className="text-caption leading-relaxed text-muted">
                Conserva la ubicación que proponga el instalador, como C:\.
                Después selecciona el ejecutable del juego. El launcher
                conservará el mismo entorno para ejecutarlo.
              </p>
              <details className="group border-t border-line pt-2">
                <summary className="flex cursor-pointer list-none items-center justify-between gap-2 text-detail text-muted focus-visible:outline focus-visible:outline-accent [&::-webkit-details-marker]:hidden">
                  Opciones avanzadas
                  <ChevronDown
                    className="h-3 w-3 shrink-0 group-open:rotate-180"
                    aria-hidden
                  />
                </summary>
                <div className="flex flex-col gap-3 pt-3">
                  <div className="flex items-center gap-2 text-caption text-muted">
                    <Checkbox
                      label="El instalador requiere Microsoft Edge WebView2"
                      checked={requireWebview2}
                      onChange={setRequireWebview2}
                      disabled={busy}
                    />
                    <span>El instalador requiere Microsoft Edge WebView2</span>
                  </div>
                  <Button
                    disabled={busy}
                    onClick={() => void pick('destination')}
                  >
                    {destinationPath
                      ? 'Cambiar carpeta externa'
                      : 'Elegir carpeta externa (opcional)'}
                  </Button>
                  {destinationPath && (
                    <>
                      <p className="text-caption font-mono text-muted break-all">
                        {destinationPath}
                      </p>
                      <Button
                        disabled={busy}
                        onClick={() => setDestinationPath('')}
                      >
                        Usar solo el entorno administrado
                      </Button>
                    </>
                  )}
                  <p className="text-caption leading-relaxed text-muted">
                    Si necesitas instalar fuera del entorno, esta carpeta se
                    vincula como R:\. Su selección en el instalador es manual;
                    no cambia el destino por sí sola.
                  </p>
                </div>
              </details>
              <Button disabled={busy} onClick={() => void begin()}>
                {busyKey === 'start'
                  ? 'Preparando instalación...'
                  : 'Preparar y abrir instalador'}
              </Button>
            </>
          )}
          {draft && (
            <>
              <p className="text-sm text-ink">
                {draft.name} · {INSTALLATION_LABELS[draft.status]}
              </p>
              <p className="text-caption text-muted">
                Runner:{' '}
                {runners.find((item) => item.path === draft.runner)?.name ??
                  basename(draft.runner)}
              </p>
              <p className="text-caption leading-relaxed text-muted">
                Puedes conservar el destino habitual del instalador en C:. El
                juego usará este mismo entorno.
              </p>
              {draft.destinationPath && (
                <p className="text-caption text-muted break-all">
                  Carpeta externa opcional (R:\): {draft.destinationPath}
                </p>
              )}
              {draft.message && (
                <p
                  role="status"
                  className="text-caption leading-relaxed text-muted"
                >
                  {draft.message}
                </p>
              )}
              {installationIsActive(draft.status) && (
                <p className="text-caption leading-relaxed text-muted">
                  Puedes cerrar esta ventana y retomar desde la lista de
                  servidores. El instalador seguirá abierto.
                </p>
              )}
              {installationIsActive(draft.status) && (
                <Button
                  disabled={busy || draft.status === 'stopping'}
                  onClick={() => void cancel()}
                >
                  Detener instalación
                </Button>
              )}
              {draft.status === 'awaitingClient' && (
                <p className="text-caption leading-relaxed text-muted">
                  Cerrar el instalador no confirma que el juego esté instalado.
                  Si no instalaste el cliente, vuelve a abrirlo. Los archivos y
                  el entorno se conservarán.
                </p>
              )}
              {installationCanRestart(draft.status) && (
                <Button disabled={busy} onClick={() => void begin()}>
                  Volver a abrir instalador
                </Button>
              )}
              {(draft.status === 'awaitingClient' ||
                draft.status === 'completed') && (
                <>
                  <Button disabled={busy} onClick={() => void scan()}>
                    Buscar ejecutables instalados
                  </Button>
                  {hasScanned && candidates.length === 0 && (
                    <p
                      role="status"
                      className="text-caption leading-relaxed text-muted"
                    >
                      No se encontraron ejecutables candidatos. Puedes volver a
                      abrir el instalador o seleccionar el cliente manualmente
                      si lo instalaste en otra ubicación.
                    </p>
                  )}
                  {candidates.length > 0 && (
                    <DarkSelect
                      value={executablePath}
                      options={[
                        { value: '', label: 'Confirmar ejecutable del juego' },
                        ...candidates.map((path) => ({
                          value: path,
                          label: basename(path),
                        })),
                      ]}
                      onChange={setExecutablePath}
                      disabled={busy}
                    />
                  )}
                  <Button disabled={busy} onClick={() => void pick('client')}>
                    {executablePath
                      ? `Juego: ${basename(executablePath)}`
                      : 'Seleccionar ejecutable del juego'}
                  </Button>
                  {executablePath && (
                    <p className="text-caption font-mono text-muted break-all">
                      {executablePath}
                    </p>
                  )}
                  <Button disabled={busy} onClick={() => void pick('patcher')}>
                    {patcherPath
                      ? `Patcher: ${basename(patcherPath)}`
                      : 'Seleccionar patcher (opcional)'}
                  </Button>
                  {patcherPath && (
                    <Button disabled={busy} onClick={() => setPatcherPath('')}>
                      Quitar patcher
                    </Button>
                  )}
                  <p className="text-caption leading-relaxed text-muted">
                    Los archivos y la configuración de esta instalación se
                    conservarán en su entorno. Una reconstrucción automática
                    estará bloqueada.
                  </p>
                  <Button
                    disabled={busy || !executablePath}
                    onClick={() => void finish()}
                  >
                    Agregar servidor instalado
                  </Button>
                </>
              )}
            </>
          )}
          {error && (
            <p role="alert" className="text-caption text-bad">
              {error}
            </p>
          )}
        </div>
      </div>
    </ModalShell>
  )
}
