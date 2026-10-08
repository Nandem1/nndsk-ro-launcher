import { describe, expect, it } from 'vitest'
import { MANAGED_RUNTIME_ID } from '../../shared/constants'
import type { RunnerInfo } from '../../shared/types'
import { resolveRunnerAfterLoad } from './settings.logic'

const proton: RunnerInfo = {
  id: MANAGED_RUNTIME_ID,
  name: 'nndsk-ro-proton 0.1.0-dev.2',
  path: '/home/user/.local/share/ro-launcher/runtime/nndsk-ro-proton-0.1.0-dev.2/proton',
}

const runners = [proton]

describe('resolveRunnerAfterLoad', () => {
  it('no migra una selección dev.1 al paquete público dev.2', () => {
    const preserved: RunnerInfo = {
      id: 'nndsk-ro-proton-0.1.0-dev.1',
      name: 'nndsk-ro-proton 0.1.0-dev.1 · local anterior',
      path: '/runtime/nndsk-ro-proton-0.1.0-dev.1/proton',
    }
    expect(resolveRunnerAfterLoad(preserved.path, [proton, preserved])).toEqual(
      {
        path: preserved.path,
        persist: false,
      },
    )
    expect(resolveRunnerAfterLoad('', [preserved, proton])).toEqual({
      path: proton.path,
      persist: true,
    })
  })
  it('elige proton preferido si no hay runner guardado', () => {
    expect(resolveRunnerAfterLoad('', runners)).toEqual({
      path: proton.path,
      persist: true,
    })
  })

  it('conserva una ruta persistida aunque ya no esté disponible', () => {
    expect(resolveRunnerAfterLoad('/custom/proton', runners)).toEqual({
      path: '/custom/proton',
      persist: false,
    })
  })

  it('conserva un runner alternativo detectado', () => {
    const alternative: RunnerInfo = {
      id: 'external:wine',
      name: 'Wine',
      path: '/usr/bin/wine',
    }
    expect(
      resolveRunnerAfterLoad(alternative.path, [proton, alternative]),
    ).toEqual({
      path: alternative.path,
      persist: false,
    })
  })

  it('prefiere nndsk-ro-proton sobre la base anterior sólo en una configuración nueva', () => {
    const previous: RunnerInfo = {
      id: 'ro-proton-cachyos-11.0-20260702-slr',
      name: 'Proton-CachyOS 11 (anterior)',
      path: '/runtime/ro-proton-cachyos-11.0-20260702-slr/proton',
    }
    expect(resolveRunnerAfterLoad('', [previous, proton])).toEqual({
      path: proton.path,
      persist: true,
    })
    expect(resolveRunnerAfterLoad(previous.path, [previous, proton])).toEqual({
      path: previous.path,
      persist: false,
    })
  })

  it('mantiene Wine 7.16 configurado al aparecer el nuevo runtime', () => {
    const legacy: RunnerInfo = {
      id: 'external:wine716',
      name: 'Wine 7.16 old-WoW64',
      path: '/runners/wine716/bin/wine',
    }
    expect(resolveRunnerAfterLoad(legacy.path, [proton, legacy])).toEqual({
      path: legacy.path,
      persist: false,
    })
  })

  it('no vuelve a persistir el runtime administrado', () => {
    expect(resolveRunnerAfterLoad(proton.path, runners)).toEqual({
      path: proton.path,
      persist: false,
    })
  })

  it('devuelve null si no hay runners', () => {
    expect(resolveRunnerAfterLoad('', [])).toBeNull()
  })

  it('conserva el runner persistido incluso si discovery no devuelve opciones', () => {
    expect(resolveRunnerAfterLoad('/usr/bin/wine', [])).toEqual({
      path: '/usr/bin/wine',
      persist: false,
    })
  })
})
