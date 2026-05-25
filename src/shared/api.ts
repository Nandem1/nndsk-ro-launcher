import { invoke } from '@tauri-apps/api/core'
import type {
  AppSettings,
  AutopotConfig,
  AutopotStatusEvent,
  DependencyStatus,
  InstallDgVoodooResult,
  RunnerInfo,
  ServerConfig,
  ServerToolsStatus,
  ToolKind,
  UninstallDgVoodooResult,
} from './types'

export const api = {
  checkDependencies: (runner: string | null) =>
    invoke<DependencyStatus>('check_dependencies', { runner }),

  setupPrefix: () => invoke<void>('setup_prefix'),

  resetPrefix: () => invoke<void>('reset_prefix'),

  launchGame: (server: ServerConfig) =>
    invoke<void>('launch_game', { server }),

  stopGame: () => invoke<void>('stop_game'),

  listServers: () => invoke<ServerConfig[]>('list_servers'),

  saveServers: (servers: ServerConfig[]) =>
    invoke<void>('save_servers', { servers }),

  loadSettings: () => invoke<AppSettings>('load_settings'),

  saveSettings: (settings: AppSettings) =>
    invoke<void>('save_settings', { settings }),

  listRunners: () => invoke<RunnerInfo[]>('list_runners'),

  scanServerTools: (server: ServerConfig) =>
    invoke<ServerToolsStatus>('scan_server_tools', { server }),

  installDgVoodoo: (server: ServerConfig) =>
    invoke<InstallDgVoodooResult>('install_dgvoodoo', { server }),

  uninstallDgVoodoo: (server: ServerConfig) =>
    invoke<UninstallDgVoodooResult>('uninstall_dgvoodoo', { server }),

  launchServerTool: (server: ServerConfig, tool: ToolKind) =>
    invoke<void>('launch_server_tool', { server, tool }),

  startAutopot: (server: ServerConfig) =>
    invoke<void>('start_autopot', { server }),

  stopAutopot: () => invoke<void>('stop_autopot'),

  updateAutopotConfig: (config: AutopotConfig) =>
    invoke<void>('update_autopot_config', { config }),

  getAutopotStatus: () => invoke<AutopotStatusEvent>('get_autopot_status'),
} as const
