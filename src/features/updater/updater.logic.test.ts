import { describe, expect, it } from 'vitest'
import type { UpdateSnapshot } from '../../shared/types'
import { clientsBlockUpdate, updateCopy } from './updater.logic'

function snapshot(
  phase: UpdateSnapshot['phase'],
  currentVersion = '0.1.0',
): UpdateSnapshot {
  return { currentVersion, phase }
}

describe('updateCopy', () => {
  it('offers install for available and clientsActive, not for other phases', () => {
    expect(updateCopy(snapshot({ kind: 'idle' })).canInstall).toBe(false)
    expect(updateCopy(snapshot({ kind: 'current' })).canInstall).toBe(false)
    expect(
      updateCopy(
        snapshot({
          kind: 'available',
          release: { version: '0.2.0', publishedAt: null, notes: null },
        }),
      ).canInstall,
    ).toBe(true)
    expect(
      updateCopy(
        snapshot({
          kind: 'failed',
          class: 'clientsActive',
          message: 'Cierra los clientes del juego antes de instalar',
        }),
      ).canInstall,
    ).toBe(true)
    expect(
      updateCopy(
        snapshot({
          kind: 'failed',
          class: 'unauthentic',
          message: 'firma inválida',
        }),
      ).canInstall,
    ).toBe(false)
    expect(
      updateCopy(
        snapshot({
          kind: 'readyToRestart',
          installedVersion: '0.2.0',
        }),
      ).canInstall,
    ).toBe(false)
  })

  it('names the installed version after a successful install', () => {
    expect(
      updateCopy(
        snapshot({
          kind: 'readyToRestart',
          installedVersion: '0.2.0',
        }),
      ).line,
    ).toBe('Lista · 0.2.0')
  })

  it('reports cumulative download percent against content length', () => {
    expect(
      updateCopy(
        snapshot({
          kind: 'downloading',
          release: { version: '0.2.0', publishedAt: null, notes: null },
          downloadedBytes: 25,
          contentLength: 100,
        }),
      ).detail,
    ).toBe('Progreso 25%')
  })

  it('reports downloaded bytes when content length is unknown', () => {
    expect(
      updateCopy(
        snapshot({
          kind: 'downloading',
          release: { version: '0.2.0', publishedAt: null, notes: null },
          downloadedBytes: 4096,
          contentLength: null,
        }),
      ).detail,
    ).toBe('Progreso 4096 B')
  })

  it('treats unauthentic as an error dot, not as current', () => {
    const copy = updateCopy(
      snapshot({
        kind: 'failed',
        class: 'unauthentic',
        message: 'firma inválida',
      }),
    )
    expect(copy.dot).toBe('error')
    expect(copy.line).not.toMatch(/Al día/)
  })

  it('labels notPackaged as AppImage-only', () => {
    const copy = updateCopy(
      snapshot({ kind: 'unavailable', reason: 'notPackaged' }),
    )
    expect(copy.line).toBe('Solo AppImage')
    expect(copy.detail).toBe('Solo disponible en el AppImage instalado')
    expect(copy.canCheck).toBe(false)
  })
})

describe('clientsBlockUpdate', () => {
  it('disables restart when launcher store has launching or running clients', () => {
    expect(
      clientsBlockUpdate({
        launchStatus: 'launching',
        clients: [],
      }),
    ).toBe(true)
    expect(
      clientsBlockUpdate({
        launchStatus: 'idle',
        clients: [
          {
            clientId: 'a',
            serverId: 's',
            serverName: 'S',
            status: 'running',
            pid: 1,
          },
        ],
      }),
    ).toBe(true)
    expect(
      clientsBlockUpdate({
        launchStatus: 'idle',
        clients: [],
      }),
    ).toBe(false)
  })
})
