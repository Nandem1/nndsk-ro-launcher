import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import { api } from '../../shared/api'
import { deferred } from '../../test/deferred'
import type { InstallationDraft } from '../../shared/types'
import { useInstallationsStore } from './installations.store'

const draft: InstallationDraft = {
  id: 'installation-1',
  serverId: 'server-1',
  name: 'TestRO',
  installerPath: '/downloads/setup.exe',
  destinationPath: '/games/test',
  runner: '/managed/proton',
  requireWebview2: false,
  status: 'pending',
  binding: null,
  exitCode: null,
  message: null,
}

describe('installation snapshots', () => {
  beforeEach(() =>
    useInstallationsStore.setState({ drafts: [draft], error: null }),
  )
  afterEach(() => vi.restoreAllMocks())
  it('cannot regress to a stale pending snapshot after start', async () => {
    const older = deferred<InstallationDraft[]>()
    vi.spyOn(api, 'listInstallations').mockReturnValue(older.promise)
    vi.spyOn(api, 'startInstallation').mockResolvedValue({
      ...draft,
      status: 'preparing',
    })
    const load = useInstallationsStore.getState().load()
    await useInstallationsStore.getState().start(draft.id)
    older.resolve([draft])
    await load
    expect(useInstallationsStore.getState().drafts[0].status).toBe('preparing')
  })
})
