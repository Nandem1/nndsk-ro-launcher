export const LAUNCHER_EVENTS = {
  LOG: 'ro-launcher://log',
  TOOL_LOG: 'ro-launcher://tool-log',
  PROGRESS: 'ro-launcher://progress',
  GAME_EXIT: 'ro-launcher://game-exit',
  AUTOPOT_STATUS: 'ro-launcher://autopot-status',
  AUTOBUFF_STATUS: 'ro-launcher://autobuff-status',
  SPAMMER_STATUS: 'ro-launcher://spammer-status',
} as const

/** Carpeta del compatibility tool en Steam (debe coincidir con PROTON_CACHYOS_SLR en Rust). */
export const PROTON_CACHYOS_SLR = 'proton-cachyos-slr'

/** Id generado por list_runners: `proton-{folder}`. */
export const PREFERRED_PROTON_ID = `proton-${PROTON_CACHYOS_SLR}` as const

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

export const DEFAULT_SPAMMER_CONFIG = {
  enabled: false,
  delayMs: 10,
  keys: ['F1'],
} as const

export const DEFAULT_AUTOBUFF_CONFIG = {
  enabled: false,
  delayMs: 300,
  rules: [],
} as const
