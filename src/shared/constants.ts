export const LAUNCHER_EVENTS = {
  LOG: 'ro-launcher://log',
  TOOL_LOG: 'ro-launcher://tool-log',
  PROGRESS: 'ro-launcher://progress',
  GAME_EXIT: 'ro-launcher://game-exit',
  AUTOPOT_STATUS: 'ro-launcher://autopot-status',
  AUTOBUFF_STATUS: 'ro-launcher://autobuff-status',
  SPAMMER_STATUS: 'ro-launcher://spammer-status',
} as const

/** Debe coincidir con MANAGED_RUNNER_ID en el backend. */
export const MANAGED_RUNTIME_ID = 'ro-proton-cachyos-11.0-20260702-slr' as const

/** Ruta por defecto del WINEPREFIX (solo para mensajes al usuario). */
export const DEFAULT_PREFIX_PATH = '~/.local/share/ro-launcher/prefix'

export const LEGACY_DEFAULT_WINE = '/usr/bin/wine'

export const POT_KEYS = [
  'F1',
  'F2',
  'F3',
  'F4',
  'F5',
  'F6',
  'F7',
  'F8',
  'F9',
  '1',
  '2',
  '3',
  '4',
  '5',
  '6',
  '7',
  '8',
  '9',
  '0',
] as const

export type PotKey = (typeof POT_KEYS)[number]

export const SPAMMER_FUNCTION_KEYS = [
  'F1',
  'F2',
  'F3',
  'F4',
  'F5',
  'F6',
  'F7',
  'F8',
  'F9',
] as const

export const SPAMMER_NUMBER_KEYS = [
  '1',
  '2',
  '3',
  '4',
  '5',
  '6',
  '7',
  '8',
  '9',
  '0',
] as const

export const SPAMMER_LETTER_KEY_ROWS = [
  ['Q', 'W', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P'],
  ['A', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L'],
  ['Z', 'X', 'C', 'V', 'B', 'N', 'M'],
] as const

export const SPAMMER_KEYS = [
  ...SPAMMER_FUNCTION_KEYS,
  ...SPAMMER_NUMBER_KEYS,
  ...SPAMMER_LETTER_KEY_ROWS[0],
  ...SPAMMER_LETTER_KEY_ROWS[1],
  ...SPAMMER_LETTER_KEY_ROWS[2],
] as const

export type SpammerKey = (typeof SPAMMER_KEYS)[number]

export const DEFAULT_AUTOPOT_CONFIG = {
  enabled: false,
  hpKey: 'F8',
  spKey: 'F9',
  hpPercent: 80,
  spPercent: 50,
  delayMs: 100,
  proactiveMode: false,
} as const

export const DEFAULT_GEAR_SWITCH_CONFIG = {
  enabled: false,
  switchDelayMs: 50,
  rules: [] as import('./types').GearSwitchRule[],
} as const

export const DEFAULT_SPAMMER_CONFIG = {
  enabled: false,
  delayMs: 16,
  keys: ['F1'],
  gearSwitch: DEFAULT_GEAR_SWITCH_CONFIG,
} as const

export const GEAR_SWITCH_MIN_DELAY_MS = 10
export const GEAR_SWITCH_MAX_DELAY_MS = 300

export const DEFAULT_AUTOBUFF_CONFIG = {
  enabled: false,
  delayMs: 300,
  rules: [],
} as const
