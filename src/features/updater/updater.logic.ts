import type { DotStatus } from '../../shared/ui/StatusDot'
import type { GameClientSnapshot, UpdateSnapshot } from '../../shared/types'
import type { LaunchStatus } from '../launcher/launcher.store'
import { isLauncherBusy } from '../launcher/launcher.store'

export function clientsBlockUpdate(args: {
  launchStatus: LaunchStatus
  clients: GameClientSnapshot[]
}): boolean {
  if (isLauncherBusy(args.launchStatus)) return true
  return args.clients.some(
    (client) =>
      client.status === 'launching' ||
      client.status === 'running' ||
      client.status === 'stopping',
  )
}

export function updateCopy(snapshot: UpdateSnapshot | null): {
  line: string
  detail: string | null
  dot: DotStatus
  canCheck: boolean
  canInstall: boolean
  canRelaunch: boolean
} {
  if (!snapshot) {
    return {
      line: 'Actualizaciones',
      detail: null,
      dot: 'neutral',
      canCheck: true,
      canInstall: false,
      canRelaunch: false,
    }
  }

  const version = `v${snapshot.currentVersion}`
  switch (snapshot.phase.kind) {
    case 'idle':
      return {
        line: `Actualizaciones · ${version}`,
        detail: 'Comprueba si hay una versión nueva del AppImage.',
        dot: 'neutral',
        canCheck: true,
        canInstall: false,
        canRelaunch: false,
      }
    case 'checking':
      return {
        line: `Buscando · ${version}`,
        detail: 'Consultando el canal de releases…',
        dot: 'warning',
        canCheck: false,
        canInstall: false,
        canRelaunch: false,
      }
    case 'current':
      return {
        line: `Al día · ${version}`,
        detail: 'Ya tienes la última versión publicada.',
        dot: 'ok',
        canCheck: true,
        canInstall: false,
        canRelaunch: false,
      }
    case 'available':
      return {
        line: `Disponible · ${snapshot.phase.release.version}`,
        detail: snapshot.phase.release.notes,
        dot: 'warning',
        canCheck: true,
        canInstall: true,
        canRelaunch: false,
      }
    case 'downloading': {
      const total = snapshot.phase.contentLength
      const progress =
        total && total > 0
          ? `${Math.min(100, Math.round((snapshot.phase.downloadedBytes / total) * 100))}%`
          : `${snapshot.phase.downloadedBytes} B`
      return {
        line: `Descargando · ${snapshot.phase.release.version}`,
        detail: `Progreso ${progress}`,
        dot: 'warning',
        canCheck: false,
        canInstall: false,
        canRelaunch: false,
      }
    }
    case 'readyToRestart':
      return {
        line: `Lista · ${snapshot.phase.installedVersion}`,
        detail: 'Reinicia el launcher para usar la versión instalada.',
        dot: 'ok',
        canCheck: false,
        canInstall: false,
        canRelaunch: true,
      }
    case 'failed':
      return {
        line:
          snapshot.phase.class === 'clientsActive'
            ? 'Clientes activos'
            : 'Error de actualización',
        detail:
          snapshot.phase.class === 'clientsActive'
            ? 'Cierra los clientes del juego antes de instalar'
            : snapshot.phase.message,
        dot: 'error',
        canCheck: true,
        canInstall: snapshot.phase.class === 'clientsActive',
        canRelaunch: false,
      }
    case 'unavailable':
      return {
        line: 'Solo AppImage',
        detail:
          snapshot.phase.reason === 'notPackaged'
            ? 'Solo disponible en el AppImage instalado'
            : 'Este sistema operativo no admite actualización automática',
        dot: 'neutral',
        canCheck: false,
        canInstall: false,
        canRelaunch: false,
      }
  }
}
