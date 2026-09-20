mod commands;
mod models;
mod state;
mod tools;
mod utils;

use tauri::{Manager, RunEvent};

use commands::{
    autobuff::{get_autobuff_status, start_autobuff, stop_autobuff, update_autobuff_config},
    autopot::{
        begin_autopot_level_scan, begin_autopot_map_scan, begin_autopot_memory_scan,
        cancel_autopot_memory_scan, find_autopot_name_address, get_autopot_status,
        list_client_profiles, refine_autopot_level_scan, refine_autopot_map_scan,
        refine_autopot_memory_scan, start_autopot, stop_autopot, update_autopot_config,
    },
    benchmarks::{
        begin_runtime_benchmark_capture, compare_runtime_benchmarks, delete_runtime_benchmarks,
        export_runtime_benchmark_comparison, export_runtime_benchmarks,
        finish_runtime_benchmark_capture, import_runtime_benchmark_samples,
        list_runtime_benchmarks, set_runtime_benchmark_visual_check, start_runtime_benchmark_run,
    },
    deps::check_dependencies,
    launcher::{launch_game, list_game_clients, stop_all_games, stop_game},
    observations::{
        delete_runtime_observations, export_runtime_observations, list_runtime_observations,
    },
    prefix::{reset_prefix, setup_prefix},
    runners::list_runners,
    server_tools::{install_dgvoodoo, launch_server_tool, scan_server_tools, uninstall_dgvoodoo},
    servers::{list_servers, save_servers},
    settings::{load_settings, save_settings},
    spammer::{get_spammer_status, start_spammer, stop_spammer, update_spammer_config},
    storage::take_storage_notices,
};
use state::{GameState, ServerRepository, SettingsRepository, StorageNotices};
use tools::{
    autobuff::AutobuffHandle, autopot::AutopotHandle, input::InputGateway,
    presence::PresenceHandle, spammer::SpammerHandle,
};
use utils::configure_linux_webview_env;

#[tauri::command]
async fn show_main_window(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    configure_linux_webview_env();

    let memory = tools::memory_sessions::MemorySessionRegistry::new();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(GameState {
            game: state::GameProcessHandle::new(),
            tool_lifecycle: tokio::sync::Mutex::new(()),
            autopot: AutopotHandle::new(memory.clone()),
            autobuff: AutobuffHandle::new(memory.clone()),
            spammer: SpammerHandle::new(),
            input: InputGateway::new(),
            presence: PresenceHandle::new(memory.clone()),
            sessions: tools::runner_sessions::RunnerSessionRegistry::new(),
            memory,
        })
        .manage(ServerRepository::default())
        .manage(SettingsRepository)
        .manage(StorageNotices::default())
        .invoke_handler(tauri::generate_handler![
            show_main_window,
            check_dependencies,
            launch_game,
            list_game_clients,
            stop_game,
            stop_all_games,
            list_runners,
            list_servers,
            load_settings,
            save_servers,
            scan_server_tools,
            install_dgvoodoo,
            uninstall_dgvoodoo,
            launch_server_tool,
            save_settings,
            take_storage_notices,
            setup_prefix,
            reset_prefix,
            start_autopot,
            stop_autopot,
            update_autopot_config,
            get_autopot_status,
            list_client_profiles,
            begin_autopot_memory_scan,
            refine_autopot_memory_scan,
            cancel_autopot_memory_scan,
            find_autopot_name_address,
            begin_autopot_level_scan,
            refine_autopot_level_scan,
            begin_autopot_map_scan,
            refine_autopot_map_scan,
            start_autobuff,
            stop_autobuff,
            update_autobuff_config,
            get_autobuff_status,
            start_spammer,
            stop_spammer,
            update_spammer_config,
            get_spammer_status,
            list_runtime_observations,
            export_runtime_observations,
            delete_runtime_observations,
            start_runtime_benchmark_run,
            begin_runtime_benchmark_capture,
            finish_runtime_benchmark_capture,
            import_runtime_benchmark_samples,
            set_runtime_benchmark_visual_check,
            list_runtime_benchmarks,
            compare_runtime_benchmarks,
            export_runtime_benchmarks,
            export_runtime_benchmark_comparison,
            delete_runtime_benchmarks,
        ])
        .build(tauri::generate_context!())
        .expect("error al iniciar la aplicación")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                if let Some(state) = app.try_state::<GameState>() {
                    state.presence.shutdown();
                    tauri::async_runtime::block_on(async {
                        let _ = tokio::join!(
                            state.autopot.stop(),
                            state.autobuff.stop(),
                            state.spammer.stop()
                        );
                        let _ = state.sessions.shutdown_all().await;
                    });
                    state.input.shutdown();
                }
            }
        });
}
