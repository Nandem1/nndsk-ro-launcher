export interface ProgressPayload {
  step: string
  percent: number
}

export interface LogEventPayload {
  line: string
}

export interface ExitEventPayload {
  code: number
}

export interface AppSettings {
  defaultRunner: string
}

export interface AutopotConfig {
  enabled: boolean
  hpKey: string
  spKey: string
  hpPercent: number
  spPercent: number
  delayMs: number
  profileId?: string
  hpBaseOverride?: string
}

export interface AutopotStatusEvent {
  active: boolean
  curHp: number
  maxHp: number
  curSp: number
  maxSp: number
  hpPercent: number
  spPercent: number
  characterName: string
  error?: string | null
}

export interface SpammerConfig {
  enabled: boolean
  delayMs: number
  keys: string[]
}

export interface SpammerStatusEvent {
  active: boolean
  armed: boolean
  spamming: boolean
  key: string
  delayMs: number
  cycleCount: number
  error?: string | null
}

export interface ServerConfig {
  id: string
  name: string
  executablePath: string
  patcherPath?: string
  winePrefix?: string
  runner?: string
  autopot?: AutopotConfig
  spammer?: SpammerConfig
}

export interface DependencyStatus {
  wine: boolean
  winetricks: boolean
  dxvk: boolean
  prefixConfigured: boolean
  audioOk: boolean
  audioDriver: string
  audioStack: string
  audioWarning: string | null
  autopotInputOk: boolean
  autopotInputWarning: string | null
  inputGroupOk: boolean
  inputGroupWarning: string | null
  prefixOk: boolean
  prefixWarning: string | null
  dxvkOk: boolean
  dxvkWarning: string | null
}

export type AudioStatus = Pick<
  DependencyStatus,
  'audioOk' | 'audioDriver' | 'audioStack' | 'audioWarning'
>

export type AutopotInputStatus = Pick<
  DependencyStatus,
  'autopotInputOk' | 'autopotInputWarning'
>

export type InputGroupStatus = Pick<
  DependencyStatus,
  'inputGroupOk' | 'inputGroupWarning'
>

export type PrefixStatus = Pick<DependencyStatus, 'prefixOk' | 'prefixWarning'>

export type DxvkStatus = Pick<DependencyStatus, 'dxvkOk' | 'dxvkWarning'>

export type AdvancedDepsStatus = Pick<
  DependencyStatus,
  | 'audioOk'
  | 'audioDriver'
  | 'audioStack'
  | 'audioWarning'
  | 'inputGroupOk'
  | 'inputGroupWarning'
  | 'autopotInputOk'
  | 'autopotInputWarning'
  | 'prefixOk'
  | 'prefixWarning'
  | 'dxvkOk'
  | 'dxvkWarning'
>

export interface RunnerInfo {
  id: string
  name: string
  path: string
}

export interface ToolInfo {
  found: boolean
  path: string | null
  label: string | null
}

export interface DgVoodooStatus {
  cpl: ToolInfo
  d3dimmDll: ToolInfo
  ddrawDll: ToolInfo
  conf: ToolInfo
  configured: boolean
  needsInstall: boolean
  canAutoInstall: boolean
  canUninstall: boolean
  issues: string[]
}

export interface ServerToolsStatus {
  gameDir: string
  openSetup: ToolInfo
  patcher: ToolInfo
  dgvoodoo: DgVoodooStatus
}

export interface InstallDgVoodooResult {
  installed: string[]
  status: ServerToolsStatus
}

export interface UninstallDgVoodooResult {
  removed: string[]
  status: ServerToolsStatus
}

export const TOOL_KINDS = ['opensetup', 'patcher', 'dgvoodoo'] as const

export type ToolKind = (typeof TOOL_KINDS)[number]

export function isToolKind(value: string | null): value is ToolKind {
  return TOOL_KINDS.includes(value as ToolKind)
}
