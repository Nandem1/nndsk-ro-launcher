import { AlertTriangle, Download, RefreshCw, RotateCcw } from 'lucide-react'
import { Button } from '../../shared/ui/Button'
import { StatusDot } from '../../shared/ui/StatusDot'
import { useLauncherStore } from '../launcher/launcher.store'
import { clientsBlockUpdate, updateCopy } from './updater.logic'
import { useUpdaterStore } from './updater.store'

/** Header-adjacent banner so a collapsed ingame rail cannot hide update state. */
export function UpdateBanner() {
  const snapshot = useUpdaterStore((s) => s.snapshot)
  const busy = useUpdaterStore((s) => s.busy)
  const check = useUpdaterStore((s) => s.check)
  const install = useUpdaterStore((s) => s.install)
  const relaunch = useUpdaterStore((s) => s.relaunch)
  const launchStatus = useLauncherStore((s) => s.status)
  const clients = useLauncherStore((s) => s.clients)

  if (!snapshot) return null
  const kind = snapshot.phase.kind
  if (
    kind !== 'available' &&
    kind !== 'readyToRestart' &&
    kind !== 'failed' &&
    kind !== 'downloading'
  ) {
    return null
  }

  const copy = updateCopy(snapshot)
  const clientsActive = clientsBlockUpdate({ launchStatus, clients })
  const tone =
    kind === 'failed'
      ? 'border-red-500/30 bg-red-500/10 text-red-100'
      : kind === 'readyToRestart'
        ? 'border-emerald-500/30 bg-emerald-500/10 text-emerald-100'
        : 'border-amber-500/30 bg-amber-500/10 text-amber-100'

  return (
    <div
      className={`mx-3 mt-3 flex shrink-0 items-center gap-3 rounded-lg border px-3 py-2 ${tone}`}
    >
      {kind === 'failed' ? (
        <AlertTriangle className="h-4 w-4 shrink-0" aria-hidden />
      ) : (
        <StatusDot status={copy.dot} pulse />
      )}
      <div className="min-w-0 flex-1">
        <p className="text-xs font-semibold">{copy.line}</p>
        <p className="truncate text-[10px] opacity-70">
          {clientsActive && (copy.canInstall || copy.canRelaunch)
            ? 'Cierra los clientes del juego antes de instalar'
            : (copy.detail ?? `Versión actual ${snapshot.currentVersion}`)}
        </p>
      </div>
      {copy.canInstall && (
        <Button
          variant="secondary"
          size="xs"
          disabled={clientsActive || busy}
          onClick={() => void install()}
        >
          <Download className="h-3 w-3" aria-hidden />
          Instalar
        </Button>
      )}
      {copy.canRelaunch && (
        <Button
          variant="secondary"
          size="xs"
          disabled={clientsActive || busy}
          onClick={() => void relaunch()}
        >
          <RotateCcw className="h-3 w-3" aria-hidden />
          Reiniciar
        </Button>
      )}
      {kind === 'failed' && (
        <Button
          variant="secondary"
          size="xs"
          disabled={busy}
          onClick={() => void check()}
        >
          <RefreshCw className="h-3 w-3" aria-hidden />
          Reintentar
        </Button>
      )}
    </div>
  )
}
