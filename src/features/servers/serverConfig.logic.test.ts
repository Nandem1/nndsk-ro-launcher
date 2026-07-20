import { describe, expect, it } from 'vitest'
import type { ServerConfig } from '../../shared/types'
import {
  createServerConfigDraft,
  effectivePrefixMode,
  serverFieldsFromDraft,
  textToArgs,
  validateServerConfigDraft,
} from './serverConfig.logic'

const server: ServerConfig = {
  id: 'sakura',
  name: 'SakuraRO',
  executablePath: '/games/sakura/ragexe.exe',
}

describe('server config draft', () => {
  it('creates new servers with an isolated prefix', () => {
    expect(createServerConfigDraft().prefixMode).toBe('isolated')
  })

  it('migrates every legacy prefix mode to isolation', () => {
    expect(effectivePrefixMode(server)).toBe('isolated')
    expect(
      effectivePrefixMode({ ...server, winePrefix: '/prefixes/sakura' }),
    ).toBe('isolated')
    expect(
      effectivePrefixMode({
        ...server,
        prefixMode: 'shared',
        winePrefix: '/legacy/prefix',
      }),
    ).toBe('isolated')
  })

  it('treats each non-empty line as one complete argv', () => {
    expect(textToArgs('one argument\r\n\n--flag=value\n ${username} ')).toEqual(
      ['one argument', '--flag=value', ' ${username} '],
    )
  })

  it('requires a patcher path for patcher strategy', () => {
    const draft = {
      ...createServerConfigDraft(server),
      strategy: 'patcher' as const,
    }
    expect(validateServerConfigDraft(draft)).toBe(
      'Selecciona un patcher para usar la estrategia Patcher',
    )
  })

  it('ignores legacy custom prefix fields', () => {
    const draft = {
      ...createServerConfigDraft(server),
      prefixMode: 'custom' as const,
    }
    expect(validateServerConfigDraft(draft)).toBeNull()
  })

  it('validates launch placeholders before persistence', () => {
    const draft = {
      ...createServerConfigDraft(server),
      gameArgs: '-t:${password',
    }
    expect(validateServerConfigDraft(draft)).toContain(
      'campo de lanzamiento sin cerrar',
    )
  })

  it('ignores legacy runner overrides', () => {
    const draft = {
      ...createServerConfigDraft(server),
      runner: '/opt/proton/proton',
    }
    expect(validateServerConfigDraft(draft)).toBeNull()
  })

  it('normalizes optional fields and serializes launch argv', () => {
    const fields = serverFieldsFromDraft({
      ...createServerConfigDraft(server),
      name: ' SakuraRO ',
      prefixMode: 'isolated',
      winePrefix: '/must/not/leak',
      gameArgs: '-1rag1\n${username}',
      patcherArgs: '--guest',
      requireWebview2: true,
    })

    expect(fields).toMatchObject({
      name: 'SakuraRO',
      prefixMode: 'isolated',
      winePrefix: null,
      runner: null,
      patcherPath: null,
      launch: {
        strategy: 'direct',
        gameArgs: ['-1rag1', '${username}'],
        patcherArgs: ['--guest'],
        requireWebview2: true,
      },
    })
  })

  it('normalizes legacy runtime fields while preserving patcher launch', () => {
    const draft = {
      ...createServerConfigDraft(server),
      patcherPath: '/games/sakura/SakuraRO Launcher.exe',
      prefixMode: 'custom' as const,
      winePrefix: '/prefixes/sakura',
      runner: '/opt/proton/proton',
      strategy: 'patcher' as const,
      gameArgs: '${username}\n${password}',
      patcherArgs: '--guest',
    }
    const fields = serverFieldsFromDraft(draft)

    expect(validateServerConfigDraft(draft)).toBeNull()
    expect(fields).toMatchObject({
      patcherPath: '/games/sakura/SakuraRO Launcher.exe',
      prefixMode: 'isolated',
      winePrefix: null,
      runner: null,
      launch: {
        strategy: 'patcher',
        gameArgs: ['${username}', '${password}'],
        patcherArgs: ['--guest'],
      },
    })
  })
})
