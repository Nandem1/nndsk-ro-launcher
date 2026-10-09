import { Download, RefreshCw, RotateCcw } from 'lucide-react'
import { Panel } from '../../shared/ui/Panel'
import { Button } from '../../shared/ui/Button'
import { StatusDot } from '../../shared/ui/StatusDot'
import { useLauncherStore } from '../launcher/launcher.store'
import { clientsBlockUpdate, updateCopy } from './updater.logic'
import { useUpdaterStore } from './updater.store'

export function UpdatePanel() {
  const snapshot = useUpdaterStore((s) => s.snapshot)
  const busy = useUpdaterStore((s) => s.busy)
  const error = useUpdaterStore((s) => s.error)
  const check = useUpdaterStore((s) => s.check)
  const install = useUpdaterStore((s) => s.install)
  const relaunch = useUpdaterStore((s) => s.relaunch)
  const launchStatus = useLauncherStore((s) => s.status)
  const clients = useLauncherStore((s) => s.clients)

  const copy = updateCopy(snapshot)
  const clientsActive = clientsBlockUpdate({ launchStatus, clients })
  const canInstall = copy.canInstall && !clientsActive && !busy
  const canRelaunch = copy.canRelaunch && !clientsActive && !busy

  return (
    <Panel
      title="Actualizaciones"
      compact
      tone={
        copy.dot === 'error'
          ? 'danger'
          : copy.dot === 'warning'
            ? 'warning'
            : copy.dot === 'ok'
              ? 'success'
              : 'neutral'
      }
      leading={<StatusDot status={copy.dot} pulse={copy.dot === 'warning'} />}
      action={
        <Button
          variant="ghost"
          size="xs"
          disabled={!copy.canCheck || busy}
          onClick={() => void check()}
        >
          <RefreshCw
            className={`h-3 w-3 ${busy && copy.canCheck === false ? 'animate-spin' : ''}`}
            aria-hidden
          />
          Comprobar
        </Button>
      }
      className="shrink-0"
    >
      <p className="text-caption leading-relaxed text-ink-soft">{copy.line}</p>
      {copy.detail && (
        <p className="mt-1 text-caption leading-relaxed text-muted">
          {copy.detail}
        </p>
      )}
      {clientsActive && (copy.canInstall || copy.canRelaunch) && (
        <p role="alert" className="mt-1 text-caption text-accent-light/90">
          Cierra los clientes del juego antes de instalar
        </p>
      )}
      {error && (
        <p role="alert" className="mt-1 text-caption text-bad-bright">
          {error}
        </p>
      )}
      {copy.canInstall && (
        <Button
          className="mt-2"
          variant="secondary"
          size="xs"
          block
          disabled={!canInstall}
          onClick={() => void install()}
        >
          <Download className="h-3 w-3" aria-hidden />
          Descargar e instalar
        </Button>
      )}
      {copy.canRelaunch && (
        <Button
          className="mt-2"
          variant="primary"
          size="xs"
          block
          disabled={!canRelaunch}
          onClick={() => void relaunch()}
        >
          <RotateCcw className="h-3 w-3" aria-hidden />
          Reiniciar
        </Button>
      )}
    </Panel>
  )
}
