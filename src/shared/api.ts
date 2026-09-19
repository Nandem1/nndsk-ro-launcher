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
  GameClientSnapshot,
  LevelScanProgress,
  MapScanProgress,
  InstallDgVoodooResult,
  LaunchValues,
  MemoryScanProgress,
  RunnerInfo,
  ServerConfig,
  ServerToolsStatus,
  SpammerConfig,
  SpammerStatusEvent,
  BenchmarkComparison,
  BenchmarkRunSummary,
  RuntimeObservationSummary,
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
    clientId: string,
    server: ServerConfig,
    launchValues: LaunchValues = {},
    runner: string | null = null,
  ) => {
    assertValid(validateServerConfig(server))
    return invoke<GameClientSnapshot>('launch_game', {
      clientId,
      server,
      runner,
      launchValues,
    })
  },

  listGameClients: () => invoke<GameClientSnapshot[]>('list_game_clients'),

  stopGame: (clientId: string) => invoke<void>('stop_game', { clientId }),

  stopAllGames: () => invoke<void>('stop_all_games'),

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

  listRuntimeObservations: () =>
    invoke<RuntimeObservationSummary[]>('list_runtime_observations'),

  exportRuntimeObservations: (destPath: string) =>
    invoke<{ exportedCount: number }>('export_runtime_observations', {
      destPath,
    }),

  deleteRuntimeObservations: () => invoke<void>('delete_runtime_observations'),

  startRuntimeBenchmarkRun: (input: {
    clientId: string
    server: ServerConfig
    runner: string | null
    arm: string
    sceneId: string
    loadDescriptor: string
    resolution: { width: number; height: number; fullscreen: boolean }
    warmupSeconds: number
    captureSeconds: number
    thermalDeclared?: string
  }) => invoke<BenchmarkRunSummary>('start_runtime_benchmark_run', { input }),

  beginRuntimeBenchmarkCapture: (runId: string) =>
    invoke<BenchmarkRunSummary>('begin_runtime_benchmark_capture', { runId }),

  finishRuntimeBenchmarkCapture: (runId: string) =>
    invoke<BenchmarkRunSummary>('finish_runtime_benchmark_capture', { runId }),

  importRuntimeBenchmarkSamples: (runId: string, csvPath: string) =>
    invoke<BenchmarkRunSummary>('import_runtime_benchmark_samples', {
      runId,
      csvPath,
    }),

  setRuntimeBenchmarkVisualCheck: (runId: string, status: string) =>
    invoke<BenchmarkRunSummary>('set_runtime_benchmark_visual_check', {
      runId,
      status,
    }),

  listRuntimeBenchmarks: () =>
    invoke<BenchmarkRunSummary[]>('list_runtime_benchmarks'),

  compareRuntimeBenchmarks: (leftRunIds: string[], rightRunIds: string[]) =>
    invoke<BenchmarkComparison>('compare_runtime_benchmarks', {
      input: { leftRunIds, rightRunIds },
    }),

  exportRuntimeBenchmarks: (destPath: string, runIds: string[]) =>
    invoke<{ exportedCount: number }>('export_runtime_benchmarks', {
      destPath,
      runIds,
    }),

  exportRuntimeBenchmarkComparison: (
    destPath: string,
    leftRunIds: string[],
    rightRunIds: string[],
  ) =>
    invoke<{ exportedCount: number }>('export_runtime_benchmark_comparison', {
      destPath,
      input: { leftRunIds, rightRunIds },
    }),

  deleteRuntimeBenchmarks: () => invoke<void>('delete_runtime_benchmarks'),

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

  findAutopotNameAddress: (characterName: string, hpBase?: string) =>
    invoke<DetectedNameAddress>('find_autopot_name_address', {
      characterName,
      hpBase: hpBase ?? null,
    }),

  beginAutopotLevelScan: (
    currentLevel: number,
    nameAddress?: string,
    hpBase?: string,
  ) =>
    invoke<LevelScanProgress>('begin_autopot_level_scan', {
      currentLevel,
      nameAddress: nameAddress ?? null,
      hpBase: hpBase ?? null,
    }),

  refineAutopotLevelScan: (currentLevel: number) =>
    invoke<LevelScanProgress>('refine_autopot_level_scan', { currentLevel }),

  beginAutopotMapScan: (
    mapName: string,
    nameAddress?: string,
    hpBase?: string,
    levelAddress?: string,
  ) =>
    invoke<MapScanProgress>('begin_autopot_map_scan', {
      mapName,
      nameAddress: nameAddress ?? null,
      hpBase: hpBase ?? null,
      levelAddress: levelAddress ?? null,
    }),

  refineAutopotMapScan: (mapName: string) =>
    invoke<MapScanProgress>('refine_autopot_map_scan', { mapName }),

  startSpammer: (server: ServerConfig) => {
    assertValid(validateServerConfig(server))
    return invoke<void>('start_spammer', { server })
  },

  stopSpammer: () => invoke<void>('stop_spammer'),

  updateSpammerConfig: (config: SpammerConfig) =>
    invoke<void>('update_spammer_config', { config }),

  getSpammerStatus: () => invoke<SpammerStatusEvent>('get_spammer_status'),
} as const
