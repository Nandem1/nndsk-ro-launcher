import { invoke } from '@tauri-apps/api/core'
import type {
  AppSettings,
  AutopotConfig,
  AutopotStatusEvent,
  AutobuffConfig,
  AutobuffStatusEvent,
  ClientProfile,
  DependencyStatus,
  InstallDgVoodooResult,
  RunnerInfo,
  ServerConfig,
  ServerToolsStatus,
  SpammerConfig,
  SpammerStatusEvent,
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
  checkDependencies: (runner: string | null) =>
    invoke<DependencyStatus>('check_dependencies', { runner }),

  setupPrefix: () => invoke<void>('setup_prefix'),

  resetPrefix: () => invoke<void>('reset_prefix'),

  launchGame: (server: ServerConfig) => {
    assertValid(validateServerConfig(server))
    return invoke<void>('launch_game', { server })
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

  launchServerTool: (server: ServerConfig, tool: ToolKind) => {
    assertValid(validateServerConfig(server))
    return invoke<void>('launch_server_tool', { server, tool })
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

  startSpammer: (server: ServerConfig) => {
    assertValid(validateServerConfig(server))
    return invoke<void>('start_spammer', { server })
  },

  stopSpammer: () => invoke<void>('stop_spammer'),

  updateSpammerConfig: (config: SpammerConfig) =>
    invoke<void>('update_spammer_config', { config }),

  getSpammerStatus: () => invoke<SpammerStatusEvent>('get_spammer_status'),
} as const
