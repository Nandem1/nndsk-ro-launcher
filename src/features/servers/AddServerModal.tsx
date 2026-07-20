import { useServersStore } from './servers.store'
import { ServerConfigModal } from './ServerConfigModal'

interface Props {
  onClose: () => void
}

export function AddServerModal({ onClose }: Props) {
  const addServer = useServersStore((state) => state.addServer)

  return (
    <ServerConfigModal
      mode="add"
      onClose={onClose}
      onSave={async (fields) => {
        await addServer({
          id: `server-${Date.now().toString(36)}`,
          ...fields,
        })
      }}
    />
  )
}
