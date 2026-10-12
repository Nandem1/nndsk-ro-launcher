import { useState } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { api } from '../../shared/api'
import { nameFromExePath } from '../../shared/path'
import { useAsyncAction } from '../../shared/hooks/useAsyncAction'
import { useSettingsStore } from '../settings/settings.store'
import { useServersStore } from '../servers/servers.store'
import { useInstallationsStore } from './installations.store'
import {
  initialInstallationRunner,
  installationExecutableDirectory,
  validateInstallationInput,
} from './installations.logic'

export function useInstallationWizard(
  installationId: string | undefined,
  onClose: () => void,
) {
  const [id, setId] = useState(installationId ?? '')
  const runners = useSettingsStore((state) => state.runners)
  const selectedRunner = useSettingsStore((state) => state.selectedRunner)
  const [name, setName] = useState('')
  const [installerPath, setInstallerPath] = useState('')
  const [destinationPath, setDestinationPath] = useState('')
  const [runner, setRunner] = useState(() =>
    initialInstallationRunner(runners, selectedRunner),
  )
  const [requireWebview2, setRequireWebview2] = useState(false)
  const [executablePath, setExecutablePath] = useState('')
  const [patcherPath, setPatcherPath] = useState('')
  const [candidates, setCandidates] = useState<string[]>([])
  const [hasScanned, setHasScanned] = useState(false)
  const draft = useInstallationsStore((state) =>
    state.drafts.find((item) => item.id === id),
  )
  const { run, busyKey, error } = useAsyncAction<string>()

  const pick = (kind: 'installer' | 'destination' | 'client' | 'patcher') =>
    run(kind, async () => {
      const selected = await open({
        title:
          kind === 'destination'
            ? 'Carpeta de destino del juego'
            : 'Seleccionar ejecutable',
        directory: kind === 'destination',
        multiple: false,
        ...(kind === 'destination'
          ? {}
          : { filters: [{ name: 'Ejecutable', extensions: ['exe'] }] }),
        ...(kind === 'client' || kind === 'patcher'
          ? { defaultPath: installationExecutableDirectory(draft) }
          : {}),
      })
      if (typeof selected !== 'string') return
      if (kind === 'installer') {
        setInstallerPath(selected)
        if (!name.trim()) setName(nameFromExePath(selected))
      }
      if (kind === 'destination') setDestinationPath(selected)
      if (kind === 'client') setExecutablePath(selected)
      if (kind === 'patcher') setPatcherPath(selected)
    })

  const begin = () =>
    run('start', async () => {
      const store = useInstallationsStore.getState()
      if (id) {
        await store.start(id)
        // Previous selections are no longer proof after another installer run.
        setExecutablePath('')
        setPatcherPath('')
        setCandidates([])
        setHasScanned(false)
        return
      }
      const input = {
        name: name.trim(),
        installerPath,
        destinationPath: destinationPath || null,
        runner,
        requireWebview2,
      }
      const validation = validateInstallationInput(input, runners)
      if (validation) throw new Error(validation)
      const created = await store.create(input)
      setId(created.id)
      await store.start(created.id)
    })

  const scan = () =>
    run('scan', async () => {
      setCandidates(await api.scanInstallationCandidates(id))
      setHasScanned(true)
    })
  const cancel = () =>
    run('cancel', () => useInstallationsStore.getState().cancel(id))
  const finish = () =>
    run('finish', async () => {
      if (!executablePath)
        throw new Error('Selecciona el ejecutable del juego instalado')
      await useServersStore
        .getState()
        .registerInstallation(id, executablePath, patcherPath || null)
      await useInstallationsStore.getState().load()
      onClose()
    })

  return {
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
    busy: busyKey !== null,
    error,
  }
}
