mod commands;
mod models;
mod state;
mod tools;
mod utils;

use tauri::{Manager, RunEvent};

use commands::{
    autobuff::{get_autobuff_status, start_autobuff, stop_autobuff, update_autobuff_config},
    autopot::{
        get_autopot_status, list_client_profiles, start_autopot, stop_autopot,
        update_autopot_config,
    },
    deps::check_dependencies,
    launcher::{launch_game, stop_game},
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
    autobuff::AutobuffHandle, autopot::AutopotHandle, input::InputGateway, spammer::SpammerHandle,
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

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(GameState {
            game: state::GameProcessHandle::new(),
            autopot: AutopotHandle::new(),
            autobuff: AutobuffHandle::new(),
            spammer: SpammerHandle::new(),
            input: InputGateway::new(),
        })
        .manage(ServerRepository::default())
        .manage(SettingsRepository)
        .manage(StorageNotices::default())
        .invoke_handler(tauri::generate_handler![
            show_main_window,
            check_dependencies,
            launch_game,
            stop_game,
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
            start_autobuff,
            stop_autobuff,
            update_autobuff_config,
            get_autobuff_status,
            start_spammer,
            stop_spammer,
            update_spammer_config,
            get_spammer_status,
        ])
        .build(tauri::generate_context!())
        .expect("error al iniciar la aplicación")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                if let Some(state) = app.try_state::<GameState>() {
                    tauri::async_runtime::block_on(async {
                        let _ = tokio::join!(
                            state.autopot.stop(),
                            state.autobuff.stop(),
                            state.spammer.stop()
                        );
                    });
                    state.input.shutdown();
                }
            }
        });
}
