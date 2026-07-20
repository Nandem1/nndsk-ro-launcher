import { describe, expect, it } from 'vitest'
import type { ServerConfig } from './types'
import {
  launchConfigKey,
  runtimeStatusKey,
  withResolvedRunner,
} from './resolveRunner'

const server: ServerConfig = {
  id: 'server-1',
  name: 'RO',
  executablePath: '/games/ro/ragexe.exe',
}

describe('withResolvedRunner', () => {
  it('normaliza cualquier configuración legacy al entorno administrado', () => {
    expect(
      withResolvedRunner(
        {
          ...server,
          prefixMode: 'custom',
          winePrefix: '/prefix/old',
          runner: '/opt/wine',
        },
        '/opt/proton',
      ),
    ).toMatchObject({
      prefixMode: 'isolated',
      winePrefix: null,
      runner: null,
    })
  })
})

describe('runtime snapshots', () => {
  it('incluye únicamente el runtime global administrado', () => {
    const isolated = { ...server, prefixMode: 'isolated' as const }
    expect(runtimeStatusKey(isolated, '/opt/proton-a')).not.toBe(
      runtimeStatusKey(isolated, '/opt/proton-b'),
    )

    const overridden = { ...isolated, runner: '/opt/server-runner' }
    expect(runtimeStatusKey(overridden, '/opt/proton-a')).not.toBe(
      runtimeStatusKey(overridden, '/opt/proton-b'),
    )
  })

  it('invalida el lanzamiento cuando cambian los argumentos activos', () => {
    const direct = {
      ...server,
      launch: { strategy: 'direct' as const, gameArgs: ['one'] },
    }
    expect(launchConfigKey(direct, '/opt/proton')).not.toBe(
      launchConfigKey(
        { ...direct, launch: { ...direct.launch, gameArgs: ['two'] } },
        '/opt/proton',
      ),
    )
  })
})
