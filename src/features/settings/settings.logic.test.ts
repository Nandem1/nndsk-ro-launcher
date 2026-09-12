import { describe, expect, it } from 'vitest'
import { MANAGED_RUNTIME_ID } from '../../shared/constants'
import type { RunnerInfo } from '../../shared/types'
import { resolveRunnerAfterLoad } from './settings.logic'

const proton: RunnerInfo = {
  id: MANAGED_RUNTIME_ID,
  name: 'proton-cachyos-slr',
  path: '/home/user/.steam/.../proton-cachyos-slr/proton',
}

const runners = [proton]

describe('resolveRunnerAfterLoad', () => {
  it('elige proton preferido si no hay runner guardado', () => {
    expect(resolveRunnerAfterLoad('', runners)).toEqual({
      path: proton.path,
      persist: true,
    })
  })

  it('migra una ruta que ya no está disponible al runtime administrado', () => {
    expect(resolveRunnerAfterLoad('/custom/proton', runners)).toEqual({
      path: proton.path,
      persist: true,
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

  it('no vuelve a persistir el runtime administrado', () => {
    expect(resolveRunnerAfterLoad(proton.path, runners)).toEqual({
      path: proton.path,
      persist: false,
    })
  })

  it('devuelve null si no hay runners', () => {
    expect(resolveRunnerAfterLoad('', [])).toBeNull()
  })
})
