import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react'
import { api } from '../../shared/api'
import { toErrorMessage } from '../../shared/errors'
import { useAsyncAction } from '../../shared/hooks/useAsyncAction'
import { runtimeConfigKey, runtimeStatusKey } from '../../shared/resolveRunner'
import { isToolKind } from '../../shared/types'
import type {
  ServerConfig,
  ServerToolsStatus,
  ToolKind,
} from '../../shared/types'
import { useSettingsStore } from '../settings/settings.store'

type ActionKey = ToolKind | 'install-dgvoodoo' | 'uninstall-dgvoodoo'

interface ScanState {
  key: string
  status: ServerToolsStatus
}

function toolsKey(server: ServerConfig | null): string {
  return server ? JSON.stringify([runtimeConfigKey(server), server.name]) : ''
}

export function useServerTools(server: ServerConfig | null) {
  const selectedRunner = useSettingsStore((s) => s.selectedRunner)
  const currentKey = toolsKey(server)
  const currentActionKey = runtimeStatusKey(server, selectedRunner)
  const currentKeyRef = useRef(currentKey)
  const scanGeneration = useRef(0)
  const [actionServerKey, setActionServerKey] = useState('')
  const [scanState, setScanState] = useState<ScanState | null>(null)
  const [refreshing, setRefreshing] = useState(false)
  const [scanError, setScanError] = useState<string | null>(null)
  const { error, setError, run, isBusy, busyKey } = useAsyncAction<ActionKey>()

  const status = scanState?.key === currentKey ? scanState.status : null

  useLayoutEffect(() => {
    currentKeyRef.current = currentKey
  }, [currentKey])

  const refresh = useCallback(async () => {
    const generation = ++scanGeneration.current
    const key = toolsKey(server)
    setScanState(null)
    setScanError(null)
    setRefreshing(!!server)
    if (!server) return

    try {
      const result = await api.scanServerTools(server)
      if (
        generation === scanGeneration.current &&
        currentKeyRef.current === key
      ) {
        setScanState({ key, status: result })
      }
    } catch (cause) {
      if (
        generation === scanGeneration.current &&
        currentKeyRef.current === key
      ) {
        setScanError(toErrorMessage(cause))
      }
    } finally {
      if (
        generation === scanGeneration.current &&
        currentKeyRef.current === key
      ) {
        setRefreshing(false)
      }
    }
  }, [server])

  useEffect(() => {
    setError(null)
    void refresh()
  }, [refresh, setError])

  const beginMutation = () => {
    ++scanGeneration.current
    setRefreshing(false)
    setScanError(null)
    setActionServerKey(currentActionKey)
  }

  const applyMutationStatus = (nextStatus: ServerToolsStatus) => {
    if (currentKeyRef.current === currentKey) {
      setScanState({ key: currentKey, status: nextStatus })
    }
  }

  const handleInstallDgVoodoo = async () => {
    if (!server || !status || refreshing || busyKey) return
    if (
      status.diagnostics.gepardPresent ||
      status.diagnostics.gameguardPresent
    ) {
      const confirmed = window.confirm(
        'Este cliente contiene anti-cheat. La compatibilidad de dgVoodoo depende de la versión y política del servidor.\n\n¿Confirmas que el administrador permite este wrapper?',
      )
      if (!confirmed) return
    }

    beginMutation()
    await run('install-dgvoodoo', async () => {
      const result = await api.installDgVoodoo(server)
      applyMutationStatus(result.status)
    })
  }

  const handleUninstallDgVoodoo = async () => {
    if (!server || !status || refreshing || busyKey) return

    const confirmed = window.confirm(
      '¿Desinstalar dgVoodoo de esta carpeta?\n\nSólo se quitarán wrappers sin modificar, se preservará una configuración editada y se restaurarán los originales respaldados.',
    )
    if (!confirmed) return

    beginMutation()
    await run('uninstall-dgvoodoo', async () => {
      const result = await api.uninstallDgVoodoo(server)
      applyMutationStatus(result.status)
    })
  }

  const handleOpen = async (tool: ToolKind) => {
    if (!server || !status || refreshing || busyKey) return
    setActionServerKey(currentActionKey)
    await run(tool, async () => {
      await api.launchServerTool(server, tool, selectedRunner || null)
    })
  }

  const actionError = actionServerKey === currentActionKey ? error : null

  return {
    status,
    loading: refreshing,
    error: scanError ?? actionError,
    opening: isToolKind(busyKey) ? busyKey : null,
    installingDgVoodoo: isBusy('install-dgvoodoo'),
    uninstallingDgVoodoo: isBusy('uninstall-dgvoodoo'),
    busy: refreshing || busyKey !== null,
    refresh,
    handleInstallDgVoodoo,
    handleUninstallDgVoodoo,
    handleOpen,
  }
}
