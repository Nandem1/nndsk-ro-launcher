import { describe, expect, it } from 'vitest'
import {
  extractLaunchFields,
  isSensitiveLaunchKey,
  requiredLaunchFields,
  validateLaunchValues,
  validateServerConfig,
  validateServers,
} from './contracts'
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

  it('mantiene configs legacy sin modo de prefijo ni launch', () => {
    expect(validateServerConfig(server)).toBeNull()
  })

  it('valida modos shared, isolated y custom sin ambigüedad', () => {
    expect(
      validateServerConfig({ ...server, prefixMode: 'isolated' }),
    ).toBeNull()
    expect(
      validateServerConfig({
        ...server,
        prefixMode: 'custom',
        winePrefix: '/prefixes/test',
      }),
    ).toBeNull()
    expect(validateServerConfig({ ...server, prefixMode: 'custom' })).toBe(
      'El modo de prefijo custom requiere un WINEPREFIX',
    )
    expect(
      validateServerConfig({
        ...server,
        prefixMode: 'shared',
        winePrefix: '/prefixes/test',
      }),
    ).toBe(
      'winePrefix sólo puede definirse cuando prefixMode es custom o legacy',
    )
  })

  it('requiere patcherPath para la estrategia patcher', () => {
    expect(
      validateServerConfig({
        ...server,
        launch: { strategy: 'patcher' },
      }),
    ).toBe('El inicio mediante patcher requiere configurar patcherPath')
  })

  it('acepta el override manual de WebView2 y rechaza tipos inválidos', () => {
    expect(
      validateServerConfig({
        ...server,
        launch: { strategy: 'direct', requireWebview2: true },
      }),
    ).toBeNull()
    expect(
      validateServerConfig({
        ...server,
        launch: {
          strategy: 'direct',
          requireWebview2: 'yes',
        } as never,
      }),
    ).toBe('El override de WebView2 no es válido')
  })

  it('extrae campos únicos sólo de la estrategia activa', () => {
    const launch = {
      strategy: 'direct' as const,
      gameArgs: ['-t:${password}', '${userId}', 'again:${password}'],
      patcherArgs: ['${patcherToken}'],
    }
    expect(requiredLaunchFields(launch)).toEqual(['password', 'userId'])
    expect(extractLaunchFields(launch.patcherArgs)).toEqual(['patcherToken'])
  })

  it('valida valores efímeros sin incluir secretos en errores', () => {
    const launch = {
      strategy: 'direct' as const,
      gameArgs: ['-t:${password}', '${userId}'],
    }
    expect(
      validateLaunchValues(launch, {
        password: 'hidden-value',
        userId: 'alice',
      }),
    ).toBeNull()
    expect(validateLaunchValues(launch, { password: 'hidden-value' })).toBe(
      "Falta el valor del campo de lanzamiento '${userId}'",
    )
    const nulError = validateLaunchValues(launch, {
      password: 'hidden\0value',
      userId: 'alice',
    })
    expect(nulError).not.toContain('hidden')
  })

  it('rechaza placeholders malformados o con keys inválidas', () => {
    expect(() => extractLaunchFields(['${password'])).toThrow(
      'campo de lanzamiento sin cerrar',
    )
    expect(() => extractLaunchFields(['${1password}'])).toThrow(
      'La clave de lanzamiento',
    )
  })

  it('identifica nombres sensibles de forma conservadora', () => {
    expect(isSensitiveLaunchKey('password')).toBe(true)
    expect(isSensitiveLaunchKey('loginToken')).toBe(true)
    expect(isSensitiveLaunchKey('userId')).toBe(false)
    expect(isSensitiveLaunchKey('bypassMode')).toBe(false)
  })
})
