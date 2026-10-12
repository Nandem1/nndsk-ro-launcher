import { describe, expect, it } from 'vitest'
import {
  initialInstallationRunner,
  installationCanRestart,
  validateInstallationInput,
} from './installations.logic'

const runners = [
  { id: 'proton', name: 'Proton', path: '/managed/proton' },
  { id: 'wine', name: 'Wine 7.16', path: '/managed/wine' },
]

describe('installer selection', () => {
  it('allows the default destination and only restarts unconfirmed installations', () => {
    expect(
      validateInstallationInput(
        {
          name: 'Test',
          installerPath: '/downloads/setup.exe',
          runner: '/managed/proton',
          requireWebview2: false,
        },
        runners,
      ),
    ).toBeNull()
    for (const status of [
      'pending',
      'failed',
      'interrupted',
      'awaitingClient',
    ] as const) {
      expect(installationCanRestart(status)).toBe(true)
    }
    for (const status of [
      'preparing',
      'running',
      'stopping',
      'completed',
    ] as const) {
      expect(installationCanRestart(status)).toBe(false)
    }
  })
  it('retains a supported default and excludes saved legacy paths from new installations', () => {
    expect(initialInstallationRunner(runners, '/managed/wine')).toBe(
      '/managed/wine',
    )
    expect(initialInstallationRunner(runners, '/usr/bin/wine')).toBe(
      '/managed/proton',
    )
    const input = {
      name: 'Test',
      installerPath: '/downloads/Setup.EXE',
      destinationPath: '/games/test',
      runner: '/usr/bin/wine',
      requireWebview2: false,
    }
    expect(validateInstallationInput(input, runners)).toContain('administrados')
    expect(
      validateInstallationInput(
        { ...input, runner: '/managed/proton' },
        runners,
      ),
    ).toBeNull()
    expect(
      validateInstallationInput(
        { ...input, destinationPath: '/', runner: '/managed/proton' },
        runners,
      ),
    ).toContain('carpeta')
  })
})
