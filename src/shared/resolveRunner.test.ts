import { describe, expect, it } from 'vitest'
import type { ServerConfig } from './types'
import {
  launchConfigKey,
  runtimeStatusKey,
  withResolvedRunner,
  runnerSelectionOptions,
} from './resolveRunner'

const server: ServerConfig = {
  id: 'server-1',
  name: 'RO',
  executablePath: '/games/ro/ragexe.exe',
}

describe('runnerSelectionOptions', () => {
  const runners = [
    { id: 'nndsk', name: 'nndsk-ro-proton', path: '/managed/nndsk/proton' },
    {
      id: 'wine716',
      name: 'Wine 7.16 Staging/TkG amd64',
      path: '/managed/wine716/bin/wine',
    },
  ]
  it('ofrece sólo el catálogo sin inventar rutas externas', () => {
    expect(runnerSelectionOptions(runners, '')).toEqual([
      { value: runners[0].path, label: runners[0].name },
      { value: runners[1].path, label: runners[1].name },
    ])
    expect(runnerSelectionOptions(runners, runners[1].path)).toHaveLength(2)
  })
  it('conserva la selección guardada sin relabelar su path ni añadir otros runners', () => {
    const options = runnerSelectionOptions(runners, '/old/wine/bin/wine')
    expect(options).toHaveLength(3)
    expect(options[2]).toEqual({
      value: '/old/wine/bin/wine',
      label: 'Selección anterior conservada · /old/wine/bin/wine',
    })
    expect(runners).toHaveLength(2)
    expect(
      withResolvedRunner(
        { ...server, runner: options[2].value },
        runners[0].path,
      ).runner,
    ).toBe('/old/wine/bin/wine')
  })
})

describe('withResolvedRunner', () => {
  it('normaliza el prefix y conserva un runner explícito por servidor', () => {
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
      runner: '/opt/wine',
    })
  })

  it('materializa el runner predeterminado para que las herramientas usen el mismo prefix', () => {
    expect(withResolvedRunner(server, '/opt/proton/proton')).toMatchObject({
      prefixMode: 'isolated',
      winePrefix: null,
      runner: '/opt/proton/proton',
    })
  })
})

describe('runtime snapshots', () => {
  it('prefiere el runner del servidor y lo incluye en el snapshot', () => {
    const isolated = { ...server, prefixMode: 'isolated' as const }
    expect(runtimeStatusKey(isolated, '/opt/proton-a')).not.toBe(
      runtimeStatusKey(isolated, '/opt/proton-b'),
    )

    const overridden = { ...isolated, runner: '/opt/server-runner' }
    expect(runtimeStatusKey(overridden, '/opt/proton-a')).toBe(
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
