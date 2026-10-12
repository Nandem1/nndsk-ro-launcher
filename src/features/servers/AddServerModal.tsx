import { useServersStore } from './servers.store'
import { ServerConfigModal } from './ServerConfigModal'
import { useState } from 'react'
import { InstallationWizard } from '../installations/InstallationWizard'

interface Props {
  onClose: () => void
}

export function AddServerModal({ onClose }: Props) {
  const [installing, setInstalling] = useState(false)
  const addServer = useServersStore((state) => state.addServer)

  if (installing) return <InstallationWizard onClose={onClose} />

  return (
    <ServerConfigModal
      mode="add"
      onClose={onClose}
      onInstall={() => setInstalling(true)}
      onSave={async (fields) => {
        await addServer({
          id: `server-${Date.now().toString(36)}`,
          ...fields,
        })
      }}
    />
  )
}
