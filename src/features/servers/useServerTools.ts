import { useCallback, useEffect, useState } from 'react'
import { api } from '../../shared/api'
import { useAsyncAction } from '../../shared/hooks/useAsyncAction'
import { isToolKind } from '../../shared/types'
import type {
  ServerConfig,
  ServerToolsStatus,
  ToolKind,
} from '../../shared/types'
import { useSettingsStore } from '../settings/settings.store'
import { withResolvedRunner } from '../../shared/resolveRunner'

type ActionKey =
  ToolKind | 'refresh' | 'install-dgvoodoo' | 'uninstall-dgvoodoo'

export function useServerTools(server: ServerConfig | null) {
  const selectedRunner = useSettingsStore((s) => s.selectedRunner)
  const [status, setStatus] = useState<ServerToolsStatus | null>(null)
  const { error, setError, run, isBusy, busyKey } = useAsyncAction<ActionKey>()

  const refresh = useCallback(async () => {
    if (!server) {
      setStatus(null)
      setError(null)
      return
    }

    const ok = await run('refresh', async () => {
      const result = await api.scanServerTools(server)
      setStatus(result)
    })
    if (!ok) setStatus(null)
  }, [server, run, setError])

  useEffect(() => {
    refresh()
  }, [refresh])

  const handleInstallDgVoodoo = async () => {
    if (!server) return

    await run('install-dgvoodoo', async () => {
      const result = await api.installDgVoodoo(server)
      setStatus(result.status)
    })
  }

  const handleUninstallDgVoodoo = async () => {
    if (!server) return

    const confirmed = window.confirm(
      '¿Desinstalar dgVoodoo de esta carpeta?\n\nSe eliminarán D3DImm.dll, DDraw.dll, dgVoodoo.conf y dgVoodooCpl.exe.',
    )
    if (!confirmed) return

    await run('uninstall-dgvoodoo', async () => {
      const result = await api.uninstallDgVoodoo(server)
      setStatus(result.status)
    })
  }

  const handleOpen = async (tool: ToolKind) => {
    if (!server) return

    await run(tool, async () => {
      await api.launchServerTool(
        withResolvedRunner(server, selectedRunner),
        tool,
      )
    })
  }

  return {
    status,
    loading: isBusy('refresh'),
    error,
    opening: isToolKind(busyKey) ? busyKey : null,
    installingDgVoodoo: isBusy('install-dgvoodoo'),
    uninstallingDgVoodoo: isBusy('uninstall-dgvoodoo'),
    refresh,
    handleInstallDgVoodoo,
    handleUninstallDgVoodoo,
    handleOpen,
  }
}
