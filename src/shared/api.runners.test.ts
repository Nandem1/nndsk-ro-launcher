import { afterEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { api } from './api'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

describe('managed runtime import IPC', () => {
  afterEach(() => vi.clearAllMocks())

  it('sends the absolute archive path using the Rust command field name', async () => {
    vi.mocked(invoke).mockResolvedValue('/runtime/nndsk-ro-proton/proton')
    expect(
      await api.importManagedRuntimeArchive('/tmp/My Runtime/runtime.tar.zst'),
    ).toBe('/runtime/nndsk-ro-proton/proton')
    expect(invoke).toHaveBeenCalledWith('import_managed_runtime_archive', {
      archivePath: '/tmp/My Runtime/runtime.tar.zst',
    })
  })
})
