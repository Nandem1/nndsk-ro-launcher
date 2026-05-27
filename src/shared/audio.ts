import type { AudioStatus, DependencyStatus } from './types'

export function audioFromDeps(deps: DependencyStatus): AudioStatus {
  return {
    audioOk: deps.audioOk,
    audioDriver: deps.audioDriver,
    audioStack: deps.audioStack,
    audioWarning: deps.audioWarning,
  }
}

export function audioDriverLabel(driver: string): string {
  switch (driver) {
    case 'pulse':
      return 'PulseAudio'
    case 'alsa':
      return 'ALSA'
    default:
      return 'sin driver'
  }
}

export function audioStatusLabel(driver: string, stack?: string): string {
  if (driver === 'alsa' && stack === 'pipewire') return 'ALSA · PipeWire'
  if (driver === 'pulse' && stack === 'pipewire') return 'Pulse · PipeWire'
  return audioDriverLabel(driver)
}
