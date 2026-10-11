import { useEffect, useId, useRef, useState } from 'react'
import type { EnvironmentAssessment } from '../../shared/types'
import { Button } from '../../shared/ui/Button'
import { ModalShell, modalSurfaceClasses } from '../../shared/ui/ModalShell'
import { StatusDot } from '../../shared/ui/StatusDot'
import {
  ENVIRONMENT_GROUPS,
  environmentSummary,
  issueScope,
} from './environment.presentation'

interface Props {
  serverName: string
  assessment: EnvironmentAssessment | null
  checking: boolean
  busy: boolean
  clientsActive?: boolean
  error: string | null
  onClose: () => void
  onRecheck: () => void
  onPrepare: () => void
}

export function EnvironmentHelpDialog({
  serverName,
  assessment,
  checking,
  busy,
  clientsActive = false,
  error,
  onClose,
  onRecheck,
  onPrepare,
}: Props) {
  const titleId = useId()
  const dialogRef = useRef<HTMLDivElement>(null)
  const [copyStatus, setCopyStatus] = useState<string | null>(null)
  const guide = assessment?.actions.find(
    (action) => action.kind === 'hostGuide',
  )
  const packages = guide?.kind === 'hostGuide' ? guide.packages : []
  const canPrepare =
    assessment?.actions.some((action) => action.kind === 'prepare') ?? false

  useEffect(() => {
    const previous = document.activeElement
    dialogRef.current
      ?.querySelector<HTMLButtonElement>('[data-environment-close]')
      ?.focus()
    const handleKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault()
        onClose()
      }
      if (event.key !== 'Tab') return
      const controls = dialogRef.current?.querySelectorAll<HTMLElement>(
        'button:not([disabled]), summary, [tabindex="0"]',
      )
      if (!controls?.length) return
      const first = controls[0]
      const last = controls[controls.length - 1]
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault()
        first.focus()
      }
    }
    document.addEventListener('keydown', handleKey)
    return () => {
      document.removeEventListener('keydown', handleKey)
      if (previous instanceof HTMLElement && previous.isConnected)
        previous.focus()
    }
  }, [onClose])

  const copyPackages = async () => {
    try {
      await navigator.clipboard.writeText(packages.join(' '))
      setCopyStatus('Lista de paquetes copiada')
    } catch {
      setCopyStatus(
        'No se pudo copiar. Puedes seleccionar la lista de paquetes.',
      )
    }
  }

  return (
    <ModalShell
      layer="launch"
      role="presentation"
      onMouseDown={(event) => {
        if (event.currentTarget === event.target) onClose()
      }}
    >
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-busy={checking || busy}
        className={`w-full max-w-xl max-h-[85vh] ${modalSurfaceClasses('glass')}`}
      >
        <div className="px-5 pt-5 pb-3 shrink-0">
          <h3 id={titleId} className="text-base font-semibold text-ink">
            Diagnóstico de {serverName}
          </h3>
          <p className="mt-1 text-sm text-muted" role="status">
            {checking
              ? 'Comprobando requisitos…'
              : assessment
                ? environmentSummary(assessment)
                : 'No se pudo completar la comprobación'}
          </p>
          {assessment?.launch === 'ready' &&
            !assessment.preparationAvailable && (
              <p className="mt-1 text-xs text-muted leading-relaxed">
                Puedes jugar. Estos requisitos se necesitan al instalar
                componentes o rearmar el entorno.
              </p>
            )}
        </div>
        <div
          data-design-panel-scroll
          className="min-h-0 overflow-y-auto px-5 pb-4 space-y-4"
        >
          {error && (
            <p className="text-sm text-bad" role="alert">
              {error}
            </p>
          )}
          {guide && packages.length > 0 && (
            <div className="notice-warn space-y-2">
              <p className="text-sm font-medium text-ink">
                Paquetes sugeridos para Arch y derivados
              </p>
              <p className="text-xs text-muted leading-relaxed">
                Instálalos con el administrador de paquetes de tu sistema.
                Después, vuelve a comprobar el entorno.
              </p>
              <p className="font-mono text-xs text-ink break-words select-text">
                {packages.join(' · ')}
              </p>
              <Button
                size="xs"
                variant="secondary"
                onClick={() => void copyPackages()}
              >
                Copiar paquetes
              </Button>
              {copyStatus && (
                <p className="text-xs text-muted" role="status">
                  {copyStatus}
                </p>
              )}
            </div>
          )}
          {ENVIRONMENT_GROUPS.map(({ domain, label }) => {
            const issues =
              assessment?.issues.filter((issue) => issue.domain === domain) ??
              []
            if (!issues.length) return null
            return (
              <section key={domain} aria-label={label} className="space-y-3">
                <h4 className="text-sm font-medium text-ink">{label}</h4>
                {issues.map((issue) => (
                  <div key={issue.code} className="space-y-1">
                    <div className="flex items-center gap-2">
                      <StatusDot
                        status={
                          issue.severity === 'pending'
                            ? 'warning'
                            : issue.severity
                        }
                      />
                      <p className="text-sm text-ink font-medium">
                        {issue.title}
                      </p>
                    </div>
                    <p className="pl-4 text-caption text-muted">
                      {issueScope(issue)}
                    </p>
                    <p className="pl-4 text-xs text-muted leading-relaxed">
                      {issue.remediation ?? issue.message}
                    </p>
                    <details className="pl-4 text-caption text-muted">
                      <summary className="cursor-pointer hover:text-ink">
                        Detalles técnicos
                      </summary>
                      <p className="pt-1 font-mono break-words">
                        {issue.code} · {issue.message}
                      </p>
                    </details>
                  </div>
                ))}
              </section>
            )
          })}
          {!checking && assessment?.issues.length === 0 && (
            <p className="text-sm text-muted">
              Las comprobaciones del entorno están completas.
            </p>
          )}
          {canPrepare && clientsActive && (
            <p className="text-xs text-muted">
              Cierra los clientes para preparar el entorno.
            </p>
          )}
        </div>
        <div className="px-5 pb-5 flex flex-wrap gap-2 shrink-0">
          <Button
            data-environment-close
            variant="secondary"
            size="dialog"
            onClick={onClose}
          >
            Cerrar
          </Button>
          <Button
            variant="secondary"
            size="dialog"
            disabled={checking || busy}
            onClick={onRecheck}
          >
            {checking ? 'Comprobando…' : 'Volver a comprobar'}
          </Button>
          {canPrepare && (
            <Button
              variant="primary"
              size="dialog"
              disabled={checking || busy || clientsActive}
              onClick={onPrepare}
            >
              Preparar entorno
            </Button>
          )}
        </div>
      </div>
    </ModalShell>
  )
}
