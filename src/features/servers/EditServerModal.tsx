import type { ServerConfig } from '../../shared/types'
import { ServerConfigModal } from './ServerConfigModal'
import { useServersStore } from './servers.store'

interface Props {
  server: ServerConfig
  onClose: () => void
}

export function EditServerModal({ server, onClose }: Props) {
  const updateServer = useServersStore((state) => state.updateServer)

  return (
    <ServerConfigModal
      mode="edit"
      server={server}
      onClose={onClose}
      onSave={async (fields) => {
        const updated = await updateServer(server.id, fields)
        if (!updated) {
          const detail = useServersStore.getState().error
          throw new Error(detail ?? 'No se pudo actualizar el servidor')
        }
      }}
    />
  )
}
