import { invoke } from '@tauri-apps/api/core'
import type {
  AppSettings,
  AutopotConfig,
  AutopotStatusEvent,
  AutobuffConfig,
  AutobuffStatusEvent,
  ClientProfile,
  DependencyStatus,
  DetectedNameAddress,
  InstallDgVoodooResult,
  LaunchValues,
  MemoryScanProgress,
  RunnerInfo,
  ServerConfig,
  ServerToolsStatus,
  SpammerConfig,
  SpammerStatusEvent,
  StorageNotice,
  ToolKind,
  UninstallDgVoodooResult,
} from './types'
import {
  validateAppSettings,
  validateServerConfig,
  validateServers,
} from './contracts'

function assertValid(error: string | null): void {
  if (error) throw new Error(error)
}

export const api = {
  checkDependencies: (
    server: ServerConfig | null,
    runner: string | null = null,
  ) => invoke<DependencyStatus>('check_dependencies', { server, runner }),

  setupPrefix: (server: ServerConfig | null, runner: string | null = null) =>
    invoke<void>('setup_prefix', { server, runner }),

  resetPrefix: (server: ServerConfig | null, runner: string | null = null) =>
    invoke<void>('reset_prefix', { server, runner }),

  launchGame: (
    server: ServerConfig,
    launchValues: LaunchValues = {},
    runner: string | null = null,
  ) => {
    assertValid(validateServerConfig(server))
    return invoke<void>('launch_game', { server, runner, launchValues })
  },

  stopGame: () => invoke<void>('stop_game'),

  listServers: () => invoke<ServerConfig[]>('list_servers'),

  saveServers: (servers: ServerConfig[]) => {
    assertValid(validateServers(servers))
    return invoke<void>('save_servers', { servers })
  },

  loadSettings: () => invoke<AppSettings>('load_settings'),

  saveSettings: (settings: AppSettings) => {
    assertValid(validateAppSettings(settings))
    return invoke<void>('save_settings', { settings })
  },

  takeStorageNotices: () => invoke<StorageNotice[]>('take_storage_notices'),

  listRunners: () => invoke<RunnerInfo[]>('list_runners'),

  scanServerTools: (server: ServerConfig) => {
    assertValid(validateServerConfig(server))
    return invoke<ServerToolsStatus>('scan_server_tools', { server })
  },

  installDgVoodoo: (server: ServerConfig) => {
    assertValid(validateServerConfig(server))
    return invoke<InstallDgVoodooResult>('install_dgvoodoo', { server })
  },

  uninstallDgVoodoo: (server: ServerConfig) => {
    assertValid(validateServerConfig(server))
    return invoke<UninstallDgVoodooResult>('uninstall_dgvoodoo', { server })
  },

  launchServerTool: (
    server: ServerConfig,
    tool: ToolKind,
    runner: string | null = null,
  ) => {
    assertValid(validateServerConfig(server))
    return invoke<void>('launch_server_tool', { server, tool, runner })
  },

  startAutopot: (server: ServerConfig) => {
    assertValid(validateServerConfig(server))
    return invoke<void>('start_autopot', { server })
  },

  stopAutopot: () => invoke<void>('stop_autopot'),

  updateAutopotConfig: (config: AutopotConfig) =>
    invoke<void>('update_autopot_config', { config }),

  getAutopotStatus: () => invoke<AutopotStatusEvent>('get_autopot_status'),

  startAutobuff: (server: ServerConfig) => {
    assertValid(validateServerConfig(server))
    return invoke<void>('start_autobuff', { server })
  },

  stopAutobuff: () => invoke<void>('stop_autobuff'),

  updateAutobuffConfig: (config: AutobuffConfig) =>
    invoke<void>('update_autobuff_config', { config }),

  getAutobuffStatus: () => invoke<AutobuffStatusEvent>('get_autobuff_status'),

  listClientProfiles: () => invoke<ClientProfile[]>('list_client_profiles'),

  beginAutopotMemoryScan: (currentHp: number) =>
    invoke<MemoryScanProgress>('begin_autopot_memory_scan', { currentHp }),

  refineAutopotMemoryScan: (currentHp: number) =>
    invoke<MemoryScanProgress>('refine_autopot_memory_scan', { currentHp }),

  cancelAutopotMemoryScan: () => invoke<void>('cancel_autopot_memory_scan'),

  findAutopotNameAddress: (characterName: string) =>
    invoke<DetectedNameAddress>('find_autopot_name_address', {
      characterName,
    }),

  startSpammer: (server: ServerConfig) => {
    assertValid(validateServerConfig(server))
    return invoke<void>('start_spammer', { server })
  },

  stopSpammer: () => invoke<void>('stop_spammer'),

  updateSpammerConfig: (config: SpammerConfig) =>
    invoke<void>('update_spammer_config', { config }),

  getSpammerStatus: () => invoke<SpammerStatusEvent>('get_spammer_status'),
} as const
