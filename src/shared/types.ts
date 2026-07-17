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

export interface StorageNotice {
  source: 'servers' | 'settings'
  kind: 'migrated' | 'recovered'
  message: string
}

export interface AutopotConfig {
  enabled: boolean
  hpKey: string
  spKey: string
  hpPercent: number
  spPercent: number
  delayMs: number
  proactiveMode: boolean
  profileId?: string
  hpBaseOverride?: string
}

export interface AutopotStatusEvent {
  active: boolean
  effectiveDelayMs: number
  curHp: number
  maxHp: number
  curSp: number
  maxSp: number
  hpPercent: number
  spPercent: number
  characterName: string
  error?: string | null
}

export interface GearSwitchRule {
  trigger: string // tecla del spammer que activa la regla
  atkKeys: string[] // se equipan al presionar
  defKeys: string[] // se equipan al soltar
}

export interface GearSwitchConfig {
  enabled: boolean
  switchDelayMs: number
  rules: GearSwitchRule[]
}

export interface SpammerConfig {
  enabled: boolean
  delayMs: number
  keys: string[]
  gearSwitch: GearSwitchConfig
}

export interface AutobuffRule {
  id: string
  label: string
  statusId: number
  key: string
  cooldownMs: number
  priority: number
  enabled: boolean
}

export interface AutobuffConfig {
  enabled: boolean
  delayMs: number
  rules: AutobuffRule[]
}

export interface AutobuffStatusEvent {
  active: boolean
  activeStatuses: number
  lastAppliedRule?: string | null
  delayMs: number
  error?: string | null
}

export interface SpammerStatusEvent {
  active: boolean
  effectiveDelayMs: number
  armed: boolean
  spamming: boolean
  key: string
  delayMs: number
  cycleCount: number
  error?: string | null
  gearMode?: 'atk' | 'def' | null
}

export interface ServerConfig {
  id: string
  name: string
  executablePath: string
  // Rust Option<T> is serialized by Tauri as null, while locally-created
  // configs may omit these fields altogether.
  patcherPath?: string | null
  winePrefix?: string | null
  runner?: string | null
  autopot?: AutopotConfig
  spammer?: SpammerConfig
  autobuff?: AutobuffConfig
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
  inputGroupOk: boolean
  inputGroupWarning: string | null
  uinputInputOk: boolean
  uinputInputWarning: string | null
  prefixOk: boolean
  prefixWarning: string | null
  dxvkOk: boolean
  dxvkWarning: string | null
}

export type AudioStatus = Pick<
  DependencyStatus,
  'audioOk' | 'audioDriver' | 'audioStack' | 'audioWarning'
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
  | 'uinputInputOk'
  | 'uinputInputWarning'
  | 'prefixOk'
  | 'prefixWarning'
  | 'dxvkOk'
  | 'dxvkWarning'
>

export interface ClientProfile {
  id: string
  label: string
  exeNames: string[]
  hpBase: number
  nameAddress: number
}

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
