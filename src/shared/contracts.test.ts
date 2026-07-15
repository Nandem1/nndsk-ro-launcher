import { describe, expect, it } from 'vitest'
import { validateServerConfig, validateServers } from './contracts'
import type { ServerConfig } from './types'

const server: ServerConfig = {
  id: 'server-1',
  name: 'Test RO',
  executablePath: '/games/test/Ragexe.exe',
}

describe('server contract', () => {
  it('acepta una configuración mínima válida', () => {
    expect(validateServerConfig(server)).toBeNull()
  })

  it('rechaza ejecutables que no son .exe', () => {
    expect(
      validateServerConfig({ ...server, executablePath: '/games/test/client' }),
    ).toBe('El ejecutable del cliente debe ser un archivo .exe')
  })

  it('acepta null en campos opcionales serializados por Tauri', () => {
    expect(
      validateServerConfig({
        ...server,
        patcherPath: null,
        winePrefix: null,
        runner: null,
      }),
    ).toBeNull()
  })

  it('rechaza ids duplicados al guardar', () => {
    expect(validateServers([server, { ...server, name: 'Otro RO' }])).toBe(
      "El identificador 'server-1' está duplicado",
    )
  })
})
