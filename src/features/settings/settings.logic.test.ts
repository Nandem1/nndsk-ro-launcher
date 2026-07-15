import { describe, expect, it } from 'vitest'
import {
  LEGACY_DEFAULT_WINE,
  PREFERRED_PROTON_ID,
} from '../../shared/constants'
import type { RunnerInfo } from '../../shared/types'
import { resolveRunnerAfterLoad } from './settings.logic'

const proton: RunnerInfo = {
  id: PREFERRED_PROTON_ID,
  name: 'proton-cachyos-slr',
  path: '/home/user/.steam/.../proton-cachyos-slr/proton',
}

const wine: RunnerInfo = {
  id: 'wine',
  name: 'Wine',
  path: LEGACY_DEFAULT_WINE,
}

const runners = [proton, wine]

describe('resolveRunnerAfterLoad', () => {
  it('elige proton preferido si no hay runner guardado', () => {
    expect(resolveRunnerAfterLoad('', runners)).toEqual({
      path: proton.path,
      persist: false,
    })
  })

  it('mantiene el runner actual si ya está configurado', () => {
    expect(resolveRunnerAfterLoad('/custom/proton', runners)).toEqual({
      path: '/custom/proton',
      persist: false,
    })
  })

  it('migra desde wine legacy al proton preferido', () => {
    expect(resolveRunnerAfterLoad(LEGACY_DEFAULT_WINE, runners)).toEqual({
      path: proton.path,
      persist: true,
    })
  })

  it('conserva wine legacy si no hay proton disponible', () => {
    expect(resolveRunnerAfterLoad(LEGACY_DEFAULT_WINE, [wine])).toEqual({
      path: LEGACY_DEFAULT_WINE,
      persist: false,
    })
  })

  it('devuelve null si no hay runners', () => {
    expect(resolveRunnerAfterLoad('', [])).toBeNull()
  })
})
